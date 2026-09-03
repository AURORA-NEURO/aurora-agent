param(
    [string]$OutputPath = "data/neurosurgery/glioma_public_snapshot.json",
    [string[]]$GdcProjectIds = @("TCGA-GBM"),
    [int]$TrialPageSize = 5,
    [int]$PortalStudyLimit = 7,
    [int]$PubMedLimit = 20,
    [string]$PubMedTerm = 'glioblastoma AND (molecular OR genomic)',
    [string]$PubMedSourceId = 'pubmed_glioblastoma',
    [string[]]$PortalStudyIds = @(
        "gbm_mayo_pdx_sarkaria_2019",
        "gbm_cptac_2021",
        "gbm_columbia_2019",
        "gbm_iatlas_prins_2019",
        "gbm_tcga_pub2013",
        "gbm_tcga_pub",
        "gbm_tcga_gdc"
    ),
    [int]$TimeoutSec = 30
)

$ErrorActionPreference = "Stop"

if ($TrialPageSize -lt 1 -or $TrialPageSize -gt 100) {
    throw "TrialPageSize must be between 1 and 100"
}
if ($GdcProjectIds.Count -lt 1 -or $GdcProjectIds.Count -gt 16) {
    throw "GdcProjectIds must contain between 1 and 16 public GDC project IDs"
}
foreach ($gdcProjectId in $GdcProjectIds) {
    if ([string]::IsNullOrWhiteSpace($gdcProjectId) -or $gdcProjectId -notmatch '^TCGA-[A-Z0-9-]+$') {
        throw "GdcProjectIds must use the allow-listed TCGA project-id shape (for example TCGA-GBM)"
    }
}
if ($PortalStudyLimit -lt 1 -or $PortalStudyLimit -gt 100) {
    throw "PortalStudyLimit must be between 1 and 100"
}
if ($PortalStudyIds.Count -lt 1 -or $PortalStudyIds.Count -gt 100) {
    throw "PortalStudyIds must contain between 1 and 100 public cBioPortal study IDs"
}
if ($PubMedLimit -lt 1 -or $PubMedLimit -gt 50) {
    throw "PubMedLimit must be between 1 and 50"
}
if ([string]::IsNullOrWhiteSpace($PubMedTerm) -or $PubMedTerm.Length -gt 512 -or $PubMedTerm -match '[\x00-\x1F]') {
    throw "PubMedTerm must be a non-empty query of at most 512 characters without control characters"
}
if ([string]::IsNullOrWhiteSpace($PubMedSourceId) -or $PubMedSourceId -notmatch '^[a-z0-9][a-z0-9_-]{2,63}$') {
    throw "PubMedSourceId must be a lowercase provenance ID of 3..64 characters"
}
$reservedSourceIds = @(
    "clinicaltrials_glioblastoma",
    "cbioportal_gbm_catalog",
    "nci_adult_cns_pdq"
) + @($GdcProjectIds | ForEach-Object { "gdc_$($_.ToLowerInvariant().Replace('-', '_'))" })
if ($reservedSourceIds -contains $PubMedSourceId) {
    throw "PubMedSourceId must be distinct from the registry, GDC, portal, and guideline source IDs"
}
if ($TimeoutSec -lt 1 -or $TimeoutSec -gt 120) {
    throw "TimeoutSec must be between 1 and 120"
}

<#
Refreshes the compact glioma snapshot from public, no-key endpoints. This script is the explicit
network boundary; the Rust agent remains offline and deterministic. The default GDC project list
preserves the GBM baseline. Pass additional allow-listed TCGA IDs (for example `-GdcProjectIds
@("TCGA-GBM","TCGA-LGG")`) to build a broader, multi-project glioma snapshot. Each project gets
its own source row and digest, so callers can distinguish population planes during replay. Pass a
broader `-PubMedTerm` and matching `-PubMedSourceId` when the literature lane should cover lower-
grade, diffuse-midline, oligodendroglial, or other glioma terminology; the source ID is retained
in every citation and its provenance row. The final content hashes are computed by the Rust CLI
over the canonical records before the bundle is written.
#>

$clinicalUri = "https://clinicaltrials.gov/api/v2/studies?query.cond=Glioblastoma&pageSize=$TrialPageSize&format=json"
$cbioUri = "https://www.cbioportal.org/api/studies?keyword=gbm"
$nciUri = "https://www.cancer.gov/types/brain/hp/adult-brain-treatment-pdq"
$pubmedTerm = [uri]::EscapeDataString($PubMedTerm)
$pubmedSearchUri = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&term=$pubmedTerm&retmax=$PubMedLimit&retmode=json&sort=pub_date"
$retrievedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$clinical = Invoke-RestMethod -Uri $clinicalUri -TimeoutSec $TimeoutSec
$clinicalRecords = @(
    $clinical.studies |
        Select-Object -First $TrialPageSize |
        ForEach-Object {
            [pscustomobject][ordered]@{
                source_id = "clinicaltrials_glioblastoma"
                nct_id = $_.protocolSection.identificationModule.nctId
                title = $_.protocolSection.identificationModule.briefTitle
                overall_status = $_.protocolSection.statusModule.overallStatus
                # ClinicalTrials.gov omits phases for observational studies. Keep that upstream
                # missingness as an empty JSON array; a PowerShell @($null) would serialize as
                # JSON null and fail the Rust Vec<String> contract.
                phases = @(
                    $_.protocolSection.designModule.phases |
                        Where-Object { $null -ne $_ -and -not [string]::IsNullOrWhiteSpace([string]$_) } |
                        ForEach-Object { [string]$_ }
                )
                last_update = $_.protocolSection.statusModule.lastUpdatePostDateStruct.date
                # Preserve registry design metadata when present. These are aggregate study
                # fields only; the Rust core keeps them optional so older snapshots remain
                # replayable and never turns absent metadata into a clinical conclusion.
                study_type = $_.protocolSection.designModule.studyType
                enrollment_count = if ($null -eq $_.protocolSection.designModule.enrollmentInfo -or $null -eq $_.protocolSection.designModule.enrollmentInfo.count) { $null } else { [int]$_.protocolSection.designModule.enrollmentInfo.count }
                intervention_names = @(
                    $_.protocolSection.armsInterventionsModule.interventions |
                        ForEach-Object { [string]$_.name } |
                        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
                        Select-Object -Unique
                )
            }
        }
)

$genomicRecords = @()
$genomicSourceRows = @()
foreach ($gdcProjectId in $GdcProjectIds) {
    $gdcProjectUri = "https://api.gdc.cancer.gov/projects/${gdcProjectId}?format=json"
    $gdcCasesFilter = [uri]::EscapeDataString("{`"op`":`"=`",`"content`":{`"field`":`"project.project_id`",`"value`":`"$gdcProjectId`"}}")
    $gdcCasesUri = "https://api.gdc.cancer.gov/cases?filters=$gdcCasesFilter&format=json&size=0"
    $gdcFilesFilter = [uri]::EscapeDataString("{`"op`":`"=`",`"content`":{`"field`":`"cases.project.project_id`",`"value`":`"$gdcProjectId`"}}")
    $gdcFilesUri = "https://api.gdc.cancer.gov/files?filters=$gdcFilesFilter&facets=data_type&format=json&size=0"
    $gdcProject = Invoke-RestMethod -Uri $gdcProjectUri -TimeoutSec $TimeoutSec
    $gdcCases = Invoke-RestMethod -Uri $gdcCasesUri -TimeoutSec $TimeoutSec
    $gdcFiles = Invoke-RestMethod -Uri $gdcFilesUri -TimeoutSec $TimeoutSec
    $gdcDataTypeCounts = @(
        $gdcFiles.data.aggregations.data_type.buckets |
            Where-Object { $null -ne $_ -and -not [string]::IsNullOrWhiteSpace([string]$_.key) -and [int]$_.doc_count -gt 0 } |
            ForEach-Object {
                [pscustomobject][ordered]@{
                    data_type = [string]$_.key
                    file_count = [int]$_.doc_count
                }
            }
    )
    if ($gdcDataTypeCounts.Count -eq 0) {
        throw "NCI GDC returned no usable file data-type facets for project $gdcProjectId"
    }
    $sourceId = "gdc_$($gdcProjectId.ToLowerInvariant().Replace('-', '_'))"
    $genomicRecords += [pscustomobject][ordered]@{
        source_id = $sourceId
        project_id = $gdcProject.data.project_id
        name = $gdcProject.data.name
        primary_site = @($gdcProject.data.primary_site)
        disease_types = @($gdcProject.data.disease_type)
        case_count = [int]$gdcCases.data.pagination.total
        data_type_counts = @($gdcDataTypeCounts)
    }
    $genomicSourceRows += [pscustomobject][ordered]@{
        source_id = $sourceId
        kind = "genomic_commons"
        authority = "NCI Genomic Data Commons"
        uri = $gdcProjectUri
        retrieved_at = $retrievedAt
        content_sha256 = ('0' * 64)
        record_count = 1
    }
}

$portal = Invoke-RestMethod -Uri $cbioUri -Headers @{ Accept = "application/json" } -TimeoutSec $TimeoutSec

function Normalize-Pmid($Value) {
    if ($null -eq $Value) { return $null }
    # cBioPortal may expose a comma-separated bibliography. Keep one stable numeric PMID;
    # the separate PubMed lane carries the complete citation set.
    $candidate = ([string]$Value -split '[,;\s]+' |
        Where-Object { $_ -match '^\d+$' } |
        Select-Object -First 1)
    if ([string]::IsNullOrWhiteSpace([string]$candidate)) { return $null }
    return [string]$candidate
}

$portalPmidCandidates = @()
foreach ($studyId in ($PortalStudyIds | Select-Object -First $PortalStudyLimit)) {
    $portalStudy = $portal | Where-Object { $_.publicStudy -eq $true -and $_.studyId -eq $studyId } | Select-Object -First 1
    $portalPmid = if ($null -eq $portalStudy) { $null } else { Normalize-Pmid $portalStudy.pmid }
    if ($null -ne $portalPmid) {
        $portalPmidCandidates += $portalPmid
    }
}

$pubmedSearch = Invoke-RestMethod -Uri $pubmedSearchUri -Headers @{ Accept = "application/json" } -TimeoutSec $TimeoutSec
$searchPubmedIds = @($pubmedSearch.esearchresult.idlist | ForEach-Object { [string]$_ })
$pubmedIds = @($portalPmidCandidates + $searchPubmedIds | Select-Object -Unique | Select-Object -First $PubMedLimit)
if ($pubmedIds.Count -eq 0) {
    throw "PubMed returned no records for the glioblastoma literature query"
}
$pubmedSummaryUri = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id=$([string]::Join(',', $pubmedIds))&retmode=json"
$pubmedSummary = Invoke-RestMethod -Uri $pubmedSummaryUri -Headers @{ Accept = "application/json" } -TimeoutSec $TimeoutSec
$pubmedFetchUri = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pubmed&id=$([string]::Join(',', $pubmedIds))&rettype=abstract&retmode=xml"
$pubmedXml = Invoke-RestMethod -Uri $pubmedFetchUri -Headers @{ Accept = "application/xml" } -TimeoutSec $TimeoutSec

function Convert-PubMedDate([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $null }
    if ($Value -match '^(\d{4})\s+([A-Za-z]{3})\s+(\d{1,2})$') {
        try {
            return ([DateTime]::ParseExact($Value, 'yyyy MMM d', [Globalization.CultureInfo]::InvariantCulture)).ToString('yyyy-MM-dd')
        }
        catch { return $null }
    }
    if ($Value -match '^(\d{4})') { return "$($Matches[1])-01-01" }
    return $null
}

function Normalize-PubMedText([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $null }
    return (($Value -replace '\s+', ' ').Trim())
}

$pubmedContentByPmid = @{}
foreach ($articleNode in @($pubmedXml.SelectNodes('//PubmedArticle'))) {
    $pmidNode = $articleNode.SelectSingleNode('./MedlineCitation/PMID')
    if ($null -eq $pmidNode) { continue }
    $abstractParts = @(
        foreach ($abstractNode in @($articleNode.SelectNodes('./MedlineCitation/Article/Abstract/AbstractText'))) {
            $part = Normalize-PubMedText ([string]$abstractNode.InnerText)
            if ([string]::IsNullOrWhiteSpace($part)) { continue }
            $label = [string]$abstractNode.GetAttribute('Label')
            if ([string]::IsNullOrWhiteSpace($label)) { $part } else { "${label}: $part" }
        }
    )
    $abstractText = if ($abstractParts.Count -eq 0) { $null } else { [string]::Join(' ', $abstractParts) }
    $abstractTruncated = $false
    if ($null -ne $abstractText -and $abstractText.Length -gt 12000) {
        $abstractText = $abstractText.Substring(0, 12000)
        $abstractTruncated = $true
    }
    while ($null -ne $abstractText -and [Text.Encoding]::UTF8.GetByteCount($abstractText) -gt 12000) {
        $abstractText = $abstractText.Substring(0, $abstractText.Length - 1)
        $abstractTruncated = $true
    }
    $pubmedContentByPmid[[string]$pmidNode.InnerText] = [pscustomobject]@{
        abstract_text = $abstractText
        abstract_truncated = $abstractTruncated
        publication_types = @($articleNode.SelectNodes('./MedlineCitation/Article/PublicationTypeList/PublicationType') | ForEach-Object { Normalize-PubMedText ([string]$_.InnerText) } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
        mesh_terms = @($articleNode.SelectNodes('./MedlineCitation/MeshHeadingList/MeshHeading/DescriptorName') | ForEach-Object { Normalize-PubMedText ([string]$_.InnerText) } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    }
}

$literatureRecords = @(
    foreach ($pmid in $pubmedIds) {
        $property = $pubmedSummary.result.PSObject.Properties[$pmid]
        if ($null -eq $property) { continue }
        $article = $property.Value
        $doiProperty = @($article.articleids | Where-Object { $_.idtype -eq 'doi' } | Select-Object -First 1)
        $content = $pubmedContentByPmid[[string]$pmid]
        # Keep singleton tag collections as JSON arrays. PowerShell's `if` expression unwraps a
        # one-item array, which would make the Rust Vec fields deserialize inconsistently.
        $publicationTypes = [System.Collections.Generic.List[string]]::new()
        $meshTerms = [System.Collections.Generic.List[string]]::new()
        if ($null -ne $content) {
            foreach ($publicationType in @($content.publication_types)) {
                if (-not [string]::IsNullOrWhiteSpace([string]$publicationType)) {
                    [void]$publicationTypes.Add([string]$publicationType)
                }
            }
            foreach ($meshTerm in @($content.mesh_terms)) {
                if (-not [string]::IsNullOrWhiteSpace([string]$meshTerm)) {
                    [void]$meshTerms.Add([string]$meshTerm)
                }
            }
        }
        [pscustomobject][ordered]@{
            source_id = $PubMedSourceId
            pmid = $pmid
            title = [string]$article.title
            journal = [string]$article.fulljournalname
            publication_date = Convert-PubMedDate ([string]$article.epubdate)
            doi = if ($doiProperty.Count -eq 0) { $null } else { [string]$doiProperty[0].value }
            abstract_text = if ($null -eq $content) { $null } else { $content.abstract_text }
            abstract_truncated = if ($null -eq $content) { $false } else { [bool]$content.abstract_truncated }
            publication_types = $publicationTypes
            mesh_terms = $meshTerms
        }
    }
)
if ($literatureRecords.Count -eq 0) {
    throw "PubMed summary returned no usable citation metadata"
}

function Get-PublicSampleCount([string]$StudyId, [int]$TimeoutSeconds) {
    try {
        # The study-level endpoint exposes an aggregate count. Do not enumerate /samples: that
        # response contains sample and patient identifiers that the snapshot never needs.
        $studyUri = "https://www.cbioportal.org/api/studies/$StudyId"
        $study = Invoke-RestMethod -Uri $studyUri -Headers @{ Accept = "application/json" } -TimeoutSec $TimeoutSeconds
        if ($null -eq $study.allSampleCount) { return $null }
        return [int]$study.allSampleCount
    }
    catch {
        # A study-level catalogue row remains useful when its optional sample endpoint is
        # temporarily unavailable; the null is preserved rather than guessed.
        return $null
    }
}

function Get-MolecularProfiles([string]$StudyId, [int]$TimeoutSeconds) {
    $profilesUri = "https://www.cbioportal.org/api/studies/$StudyId/molecular-profiles"
    # Molecular-profile metadata is the modality inventory for a study. The endpoint is public
    # and returns no patient-level values; a failed call aborts refresh so the snapshot cannot
    # silently claim complete assay coverage.
    return Invoke-RestMethod -Uri $profilesUri -Headers @{ Accept = "application/json" } -TimeoutSec $TimeoutSeconds
}

$selectedPortal = $portal | Where-Object { $_.publicStudy -eq $true }
$portalRecords = @(
    foreach ($studyId in ($PortalStudyIds | Select-Object -First $PortalStudyLimit)) {
        $study = $selectedPortal | Where-Object { $_.studyId -eq $studyId } | Select-Object -First 1
        if ($null -eq $study) {
            throw "cBioPortal public study $studyId was not present in the live catalogue"
        }
        [pscustomobject][ordered]@{
            source_id = "cbioportal_gbm_catalog"
            study_id = $study.studyId
            name = $study.name
            description = ($study.description -replace '<[^>]+>', '')
            sample_count = Get-PublicSampleCount $study.studyId $TimeoutSec
            pmid = Normalize-Pmid $study.pmid
            public_study = [bool]$study.publicStudy
        }
    }
)

$referenceRecords = @(
    [pscustomobject][ordered]@{
        source_id = "nci_adult_cns_pdq"
        # The reference identity is stable; retrieval time belongs to the source metadata.
        reference_id = "NCI-PDQ-adult-CNS"
        title = "Central Nervous System Tumors Treatment (PDQ) - Health Professional Version"
        uri = $nciUri
        publisher = "National Cancer Institute"
    }
)

$portalMolecularProfileRecords = @(
    foreach ($study in $portalRecords) {
        $profiles = Get-MolecularProfiles $study.study_id $TimeoutSec
        foreach ($profile in @($profiles)) {
            [pscustomobject][ordered]@{
                source_id = $study.source_id
                study_id = [string]$profile.studyId
                profile_id = [string]$profile.molecularProfileId
                name = [string]$profile.name
                molecular_alteration_type = [string]$profile.molecularAlterationType
                datatype = [string]$profile.datatype
                description = if ($null -eq $profile.description) { $null } else { [string]$profile.description }
                show_in_analysis = [bool]$profile.showProfileInAnalysisTab
                patient_level = [bool]$profile.patientLevel
            }
        }
    }
)
if ($portalMolecularProfileRecords.Count -eq 0) {
    throw "cBioPortal returned no molecular-profile metadata for the selected public glioma studies"
}

$sources = @(
    [pscustomobject][ordered]@{
        source_id = "clinicaltrials_glioblastoma"
        kind = "clinical_trials_registry"
        authority = "ClinicalTrials.gov / U.S. National Library of Medicine"
        uri = $clinicalUri
        retrieved_at = $retrievedAt
        content_sha256 = ('0' * 64)
        record_count = $clinicalRecords.Count
    },
    $genomicSourceRows,
    [pscustomobject][ordered]@{
        source_id = "cbioportal_gbm_catalog"
        kind = "study_portal"
        authority = "cBioPortal for Cancer Genomics"
        uri = $cbioUri
        retrieved_at = $retrievedAt
        content_sha256 = ('0' * 64)
        record_count = $portalRecords.Count + $portalMolecularProfileRecords.Count
    },
    [pscustomobject][ordered]@{
        source_id = "nci_adult_cns_pdq"
        kind = "guideline"
        authority = "National Cancer Institute"
        uri = $nciUri
        retrieved_at = $retrievedAt
        content_sha256 = ('0' * 64)
        record_count = $referenceRecords.Count
    },
    [pscustomobject][ordered]@{
        source_id = $PubMedSourceId
        kind = "literature_index"
        authority = "U.S. National Library of Medicine PubMed"
        uri = $pubmedSearchUri
        retrieved_at = $retrievedAt
        content_sha256 = ('0' * 64)
        record_count = $literatureRecords.Count
    }
)
# PowerShell preserves an array-valued expression as one nested element inside an array literal.
# Flatten the generated GDC source rows so the wire bundle remains a simple source-object list.
$sources = @(
    foreach ($source in $sources) {
        if ($source -is [System.Array]) {
            foreach ($nestedSource in $source) { $nestedSource }
        }
        else { $source }
    }
)

$bundle = [pscustomobject][ordered]@{
    schema_version = "bioprism-neurosurgery-real/0.1"
    generated_at = $retrievedAt
    synthetic_data = $false
    sources = @($sources)
    clinical_trials = @($clinicalRecords)
    genomic_projects = @($genomicRecords)
    portal_studies = @($portalRecords)
    portal_molecular_profiles = @($portalMolecularProfileRecords)
    references = @($referenceRecords)
    literature = @($literatureRecords)
}

$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = [IO.Path]::GetDirectoryName($outputFullPath)
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$candidateName = ".{0}.candidate.{1}.json" -f @(
    [IO.Path]::GetFileName($outputFullPath),
    [Guid]::NewGuid().ToString("N")
)
$temporaryPath = [IO.Path]::Combine(
    $outputDirectory,
    $candidateName
)
try {
    $jsonText = $bundle | ConvertTo-Json -Depth 20
    # Windows PowerShell's `-Encoding utf8` writes a BOM, which serde_json rejects at byte 0.
    # Write UTF-8 without a BOM so the snapshot and the hash helper share one portable format.
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($temporaryPath, $jsonText, $utf8NoBom)
    $hashJson = cargo run -p bioprism-neurosurgery --offline --quiet -- --real-data-hashes $temporaryPath
    $hashes = $hashJson | ConvertFrom-Json
    foreach ($source in $bundle.sources) {
        $property = $hashes.PSObject.Properties[$source.source_id]
        if ($null -eq $property) {
            throw "Rust hash helper did not return source $($source.source_id)"
        }
        $source.content_sha256 = $property.Value
    }
    $finalJsonText = $bundle | ConvertTo-Json -Depth 20
    # Validate the candidate before touching the live snapshot. A failed refresh therefore leaves
    # the last known-good real-data bundle intact and the candidate is removed in the finally block.
    [IO.File]::WriteAllText($temporaryPath, $finalJsonText, $utf8NoBom)
    $validation = cargo run -p bioprism-neurosurgery --offline --quiet -- --validate-real-glioma $temporaryPath
    if ($LASTEXITCODE -ne 0) {
        throw "Rust validation rejected the refreshed bundle: $validation"
    }
    if ([IO.File]::Exists($outputFullPath)) {
        # .NET on Windows requires a non-empty backup path for File.Replace. Keep the swap
        # atomic, but clean the short-lived backup immediately so a refresh never leaves stale
        # candidate/backup artifacts beside the last-known-good snapshot.
        $backupPath = "{0}.{1}.backup" -f $outputFullPath, [Guid]::NewGuid().ToString("N")
        try {
            [IO.File]::Replace($temporaryPath, $outputFullPath, $backupPath)
        }
        finally {
            Remove-Item -LiteralPath $backupPath -Force -ErrorAction SilentlyContinue
        }
    }
    else {
        [IO.File]::Move($temporaryPath, $outputFullPath)
    }
    Write-Output "Wrote $outputFullPath from ClinicalTrials.gov, NCI GDC, cBioPortal, NCI PDQ, and PubMed."
}
finally {
    Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
}
