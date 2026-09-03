param(
    [string]$OutputPath = "data/neurosurgery/neurosurgical_public_literature_snapshot.json",
    [int]$PerSpecialtyLimit = 10,
    [int]$TimeoutSec = 60
)

$ErrorActionPreference = "Stop"

if ($PerSpecialtyLimit -lt 1 -or $PerSpecialtyLimit -gt 50) {
    throw "PerSpecialtyLimit must be between 1 and 50"
}
if ($TimeoutSec -lt 1 -or $TimeoutSec -gt 120) {
    throw "TimeoutSec must be between 1 and 120"
}

<#
Refresh a bounded, cross-specialty PubMed metadata snapshot. This is the only network boundary;
the Rust bundle validator remains offline and rejects malformed, unbound, or synthetic snapshots.
The records contain citation metadata, abstracts, publication-type tags, and MeSH descriptors only.
#>

$lanes = [ordered]@{
    # Keep each lane broad enough to capture its established literature while naming the
    # specialist subtopics the offline intake vocabulary can route and review explicitly.
    glioma = '(glioma OR glioblastoma OR astrocytoma OR oligodendroglioma OR "diffuse midline glioma") AND (molecular OR genomic OR pseudoprogression OR "radiation necrosis")'
    cranial_base = '((skull base) OR (cranial base) OR petroclival OR "cavernous sinus" OR "cranial nerve" OR "CSF leak") AND (neurosurgery OR surgery)'
    craniosynostosis = '(craniosynostosis OR scaphocephaly OR plagiocephaly OR "Apert syndrome" OR "Crouzon syndrome" OR "Pfeiffer syndrome")'
    encephalocele = '(encephalocele OR meningoencephalocele OR "basal encephalocele" OR "occipital encephalocele" OR "CSF rhinorrhea")'
    spina_bifida = '((spina bifida) OR (spinal dysraphism) OR myelomeningocele OR lipomeningocele OR "tethered cord" OR "neurogenic bladder" OR diastematomyelia)'
    chiari_malformation = '((Chiari malformation) OR (craniocervical junction) OR syringomyelia OR "cine MRI" OR "CSF flow" OR "clivo-axial angle" OR "basilar invagination")'
}

$retrievedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$records = [System.Collections.Generic.List[object]]::new()
$sources = [System.Collections.Generic.List[object]]::new()
$seenPmids = [System.Collections.Generic.HashSet[string]]::new()

function Convert-PubMedDate([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $null }
    if ($Value -match '^([0-9]{4})\s+([A-Za-z]{3})\s+([0-9]{1,2})$') {
        try {
            return ([DateTime]::ParseExact($Value, 'yyyy MMM d', [Globalization.CultureInfo]::InvariantCulture)).ToString('yyyy-MM-dd')
        }
        catch { return $null }
    }
    if ($Value -match '^([0-9]{4})') { return "$($Matches[1])-01-01" }
    return $null
}

function Normalize-PubMedText([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $null }
    return (($Value -replace '\s+', ' ').Trim())
}

function Invoke-PubMed([string]$Uri) {
    # NCBI's unauthenticated E-utilities budget is three requests per second. Keep the refresh
    # usable without an API key and retry one rate-limit response after a full-second pause.
    Start-Sleep -Milliseconds 450
    try {
        return Invoke-RestMethod -Uri $Uri -Headers @{ Accept = "application/json, application/xml" } -TimeoutSec $TimeoutSec
    }
    catch {
        if ($_.Exception.Message -notmatch 'rate limit|API rate limit') { throw }
        Start-Sleep -Seconds 2
        return Invoke-RestMethod -Uri $Uri -Headers @{ Accept = "application/json, application/xml" } -TimeoutSec $TimeoutSec
    }
}

foreach ($lane in $lanes.GetEnumerator()) {
    $term = [uri]::EscapeDataString([string]$lane.Value)
    $searchUri = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&term=$term&retmax=$PerSpecialtyLimit&retmode=json&sort=pub_date"
    $search = Invoke-PubMed $searchUri
    $ids = @($search.esearchresult.idlist | ForEach-Object { [string]$_ })
    if ($ids.Count -eq 0) {
        throw "PubMed returned no records for specialty lane $($lane.Key)"
    }

    $summaryUri = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id=$([string]::Join(',', $ids))&retmode=json"
    $summary = Invoke-PubMed $summaryUri
    $fetchUri = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pubmed&id=$([string]::Join(',', $ids))&rettype=abstract&retmode=xml"
    $xml = Invoke-PubMed $fetchUri
    $contentByPmid = @{}
    foreach ($articleNode in @($xml.SelectNodes('//PubmedArticle'))) {
        $pmidNode = $articleNode.SelectSingleNode('./MedlineCitation/PMID')
        if ($null -eq $pmidNode) { continue }
        $abstractParts = [System.Collections.Generic.List[string]]::new()
        foreach ($abstractNode in @($articleNode.SelectNodes('./MedlineCitation/Article/Abstract/AbstractText'))) {
            $part = Normalize-PubMedText ([string]$abstractNode.InnerText)
            if ([string]::IsNullOrWhiteSpace([string]$part)) { continue }
            $label = [string]$abstractNode.GetAttribute('Label')
            if ([string]::IsNullOrWhiteSpace($label)) { [void]$abstractParts.Add([string]$part) }
            else { [void]$abstractParts.Add("${label}: $part") }
        }
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
        $publicationTypes = [System.Collections.Generic.List[string]]::new()
        foreach ($node in @($articleNode.SelectNodes('./MedlineCitation/Article/PublicationTypeList/PublicationType'))) {
            $value = Normalize-PubMedText ([string]$node.InnerText)
            if (-not [string]::IsNullOrWhiteSpace([string]$value)) { [void]$publicationTypes.Add([string]$value) }
        }
        $meshTerms = [System.Collections.Generic.List[string]]::new()
        foreach ($node in @($articleNode.SelectNodes('./MedlineCitation/MeshHeadingList/MeshHeading/DescriptorName'))) {
            $value = Normalize-PubMedText ([string]$node.InnerText)
            if (-not [string]::IsNullOrWhiteSpace([string]$value)) { [void]$meshTerms.Add([string]$value) }
        }
        $contentByPmid[[string]$pmidNode.InnerText] = [pscustomobject]@{
            abstract_text = $abstractText
            abstract_truncated = $abstractTruncated
            publication_types = $publicationTypes
            mesh_terms = $meshTerms
        }
    }

    # Keep source identity stable across refreshes; retrieval time belongs in metadata, not the
    # identifier used to reconcile snapshots.
    $sourceId = "pubmed_$($lane.Key)"
    $laneRecords = [System.Collections.Generic.List[object]]::new()
    foreach ($pmid in $ids) {
        # A paper returned by more than one lane is retained once, with the first retrieval lane.
        if (-not $seenPmids.Add([string]$pmid)) { continue }
        $property = $summary.result.PSObject.Properties[[string]$pmid]
        if ($null -eq $property) { continue }
        $article = $property.Value
        $doiProperty = @($article.articleids | Where-Object { $_.idtype -eq 'doi' } | Select-Object -First 1)
        $content = $contentByPmid[[string]$pmid]
        $recordPublicationTypes = [System.Collections.Generic.List[string]]::new()
        $recordMeshTerms = [System.Collections.Generic.List[string]]::new()
        if ($null -ne $content) {
            foreach ($value in @($content.publication_types)) {
                if (-not [string]::IsNullOrWhiteSpace([string]$value)) {
                    [void]$recordPublicationTypes.Add([string]$value)
                }
            }
            foreach ($value in @($content.mesh_terms)) {
                if (-not [string]::IsNullOrWhiteSpace([string]$value)) {
                    [void]$recordMeshTerms.Add([string]$value)
                }
            }
        }
        $record = [pscustomobject][ordered]@{
            source_id = $sourceId
            specialty = [string]$lane.Key
            pmid = [string]$pmid
            title = [string]$article.title
            journal = [string]$article.fulljournalname
            publication_date = Convert-PubMedDate ([string]$article.epubdate)
            doi = if ($doiProperty.Count -eq 0) { $null } else { [string]$doiProperty[0].value }
            abstract_text = if ($null -eq $content) { $null } else { $content.abstract_text }
            abstract_truncated = if ($null -eq $content) { $false } else { [bool]$content.abstract_truncated }
            publication_types = $recordPublicationTypes
            mesh_terms = $recordMeshTerms
        }
        [void]$laneRecords.Add($record)
        [void]$records.Add($record)
    }
    if ($laneRecords.Count -eq 0) {
        throw "PubMed lane $($lane.Key) produced no unique citation records"
    }
    [void]$sources.Add([pscustomobject][ordered]@{
        source_id = $sourceId
        authority = "U.S. National Library of Medicine PubMed"
        uri = $searchUri
        retrieved_at = $retrievedAt
        content_sha256 = ('0' * 64)
        record_count = $laneRecords.Count
    })
}

$bundle = [pscustomobject][ordered]@{
    schema_version = "bioprism-neurosurgery-public-literature/0.1"
    generated_at = $retrievedAt
    synthetic_data = $false
    sources = @($sources)
    records = @($records)
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
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($temporaryPath, ($bundle | ConvertTo-Json -Depth 20), $utf8NoBom)
    $hashJson = cargo run -p bioprism-neurosurgery --offline --quiet -- --public-literature-hashes $temporaryPath
    if ($LASTEXITCODE -ne 0) { throw "Rust public-literature hash helper failed: $hashJson" }
    $hashes = $hashJson | ConvertFrom-Json
    foreach ($source in $bundle.sources) {
        $property = $hashes.PSObject.Properties[$source.source_id]
        if ($null -eq $property) { throw "Rust hash helper did not return source $($source.source_id)" }
        $source.content_sha256 = $property.Value
    }
    # Validate before replacing the live snapshot so a failed network refresh cannot destroy the
    # last known-good real-literature corpus.
    [IO.File]::WriteAllText($temporaryPath, ($bundle | ConvertTo-Json -Depth 20), $utf8NoBom)
    $validation = cargo run -p bioprism-neurosurgery --offline --quiet -- --validate-public-literature $temporaryPath
    if ($LASTEXITCODE -ne 0) { throw "Rust validation rejected the refreshed snapshot: $validation" }
    if ([IO.File]::Exists($outputFullPath)) {
        [IO.File]::Replace($temporaryPath, $outputFullPath, $null)
    }
    else {
        [IO.File]::Move($temporaryPath, $outputFullPath)
    }
    Write-Output "Wrote $outputFullPath from six bounded PubMed specialty lanes."
}
finally {
    Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
}
