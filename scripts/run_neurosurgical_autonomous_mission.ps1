param(
    [Parameter(Mandatory = $true)]
    [string]$RequestPath,
    [string]$RealDataPath = "data/neurosurgery/glioma_extended_snapshot.json",
    [string[]]$GdcProjectIds = @("TCGA-GBM", "TCGA-LGG"),
    [string]$PubMedTerm = '(glioma OR glioblastoma OR astrocytoma OR oligodendroglioma OR "diffuse midline glioma") AND (molecular OR genomic OR IDH OR MGMT OR methylation)',
    [string]$PubMedSourceId = "pubmed_glioma_molecular",
    [string]$PublicLiteraturePath = "data/neurosurgery/neurosurgical_public_literature_snapshot.json",
    [string]$MissionQueryPath,
    [string]$PublicLiteratureQueryPath,
    [string]$PortfolioQueryPath,
    [string]$FreshnessQueryPath,
    [string]$CaseAssetManifestPath,
    [string]$CaseAssetManifestQueryPath,
    [string]$MissionCaseAssetReviewDispositionPath,
    [string]$CaseDicomImportPath,
    [string]$CaseFhirImportPath,
    [string]$MissionOutputPath = "work/neurosurgical-mission.json",
    [int]$MaxSessionSteps = 256,
    [switch]$SkipRefresh
)

$ErrorActionPreference = "Stop"

if ($MaxSessionSteps -lt 1 -or $MaxSessionSteps -gt 256) {
    throw "MaxSessionSteps must be between 1 and 256"
}
if (-not [IO.File]::Exists($RequestPath)) {
    throw "RequestPath does not exist: $RequestPath"
}
if ([string]::IsNullOrWhiteSpace($MissionOutputPath)) {
    throw "MissionOutputPath must not be empty"
}

<#
Run a bounded, real-data neurosurgical mission without a model provider or API key.

When refresh is enabled, the two existing refresh scripts are the only network boundary:
ClinicalTrials.gov, NCI GDC, cBioPortal, NCI PDQ, and PubMed are queried into candidate JSON
snapshots, and each candidate is validated and source-hashed before replacing its last-known-good
file. The Rust mission then binds both validated bundles, keeps them separate, emits an exact
PMID/DOI linkage audit, and includes the dual-plane evidence-synthesis ledger. No patient files,
credentials, synthetic records, or clinical actions are accepted by this runner. An optional
de-identified case-asset manifest is projected as metadata only; asset bytes are never opened.
`-GdcProjectIds` forwards an allow-listed TCGA project list to the glioma refresh (the default
includes TCGA-GBM and TCGA-LGG for a broader glioma population); `-PubMedTerm` and
`-PubMedSourceId` can narrow or widen and identify the real citation lane while retaining that
query/source identity in the snapshot. The baseline GBM-only snapshot remains available by
passing `-RealDataPath data/neurosurgery/glioma_public_snapshot.json` and the matching source
controls explicitly.
The mission also emits digest-bound ClinicalTrials.gov trial-landscape and cBioPortal
assay/profile-coverage inventories; this runner verifies both remain real, provider-free,
network-free, and human-review-gated against the same snapshot.
The evidence packet also carries a canonical PMID/normalized-DOI reconciliation ledger; this
runner verifies that the ledger remains bound to the packet's snapshot digest and preserves its
provider-free, network-free, human-review-only posture.
The independent six-specialty PubMed packet is checked with the same digest and posture rules,
so a mission cannot silently fall back to an unchecked or synthetic citation plane.
An optional `-CaseFhirImportPath` carries the same sanitized FHIR metadata through the mission
asset plane; its receipt and manifest binding are verified before persistence.
An optional `-CaseDicomImportPath` can be supplied alone or with FHIR to compose a multimodal
digest-only case projection; both child receipts are checked against the union manifest.
When a case-asset manifest is supplied, the runner also verifies that the evidence-synthesis
`case_asset_summary` report digest and coverage counts match the manifest projection before
emitting the envelope. A persisted reviewer ledger can be carried with
`-MissionCaseAssetReviewDispositionPath`; the Rust mission audit revalidates its manifest and
synthesis bindings before emitting the envelope. The mission is persisted atomically to
`-MissionOutputPath` and immediately replay-validated against the exact request and snapshots
before it is printed, so a downstream worker never receives an unchecked checkpoint.
#>

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$realRefresh = Join-Path $scriptRoot "refresh_glioma_public_data.ps1"
$literatureRefresh = Join-Path $scriptRoot "refresh_neurosurgical_public_literature.ps1"

if (-not $SkipRefresh) {
    if (-not [IO.File]::Exists($realRefresh) -or -not [IO.File]::Exists($literatureRefresh)) {
        throw "The real-data refresh scripts are missing under $scriptRoot"
    }
    # Refresh scripts validate candidates before promotion. Their progress is intentionally not
    # mixed into the JSON returned by this runner.
    $realRefreshParameters = @{
        OutputPath = $RealDataPath
        GdcProjectIds = $GdcProjectIds
        PubMedTerm = $PubMedTerm
        PubMedSourceId = $PubMedSourceId
    }
    [void](& $realRefresh @realRefreshParameters)
    [void](& $literatureRefresh -OutputPath $PublicLiteraturePath)
}

if (-not [IO.File]::Exists($RealDataPath)) {
    throw "RealDataPath does not exist: $RealDataPath"
}
if (-not [IO.File]::Exists($PublicLiteraturePath)) {
    throw "PublicLiteraturePath does not exist: $PublicLiteraturePath"
}
if (-not [string]::IsNullOrWhiteSpace($CaseFhirImportPath) -and
    -not [IO.File]::Exists($CaseFhirImportPath)) {
    throw "CaseFhirImportPath does not exist: $CaseFhirImportPath"
}
if (-not [string]::IsNullOrWhiteSpace($CaseDicomImportPath) -and
    -not [IO.File]::Exists($CaseDicomImportPath)) {
    throw "CaseDicomImportPath does not exist: $CaseDicomImportPath"
}
if ((-not [string]::IsNullOrWhiteSpace($CaseFhirImportPath) -or
     -not [string]::IsNullOrWhiteSpace($CaseDicomImportPath)) -and
    (-not [string]::IsNullOrWhiteSpace($CaseAssetManifestPath) -or
     -not [string]::IsNullOrWhiteSpace($MissionCaseAssetReviewDispositionPath))) {
    throw "CaseDicomImportPath/CaseFhirImportPath cannot be combined with a case asset manifest or disposition"
}

$cargoArguments = @(
    "run",
    "-p", "bioprism-neurosurgery",
    "--offline",
    "--quiet",
    "--",
    "--mission",
    "--real-glioma", $RealDataPath,
    "--public-literature", $PublicLiteraturePath,
    "--max-session-steps", [string]$MaxSessionSteps
)
if (-not [string]::IsNullOrWhiteSpace($MissionQueryPath)) {
    if (-not [IO.File]::Exists($MissionQueryPath)) { throw "MissionQueryPath does not exist: $MissionQueryPath" }
    $cargoArguments += @("--mission-query", $MissionQueryPath)
}
if (-not [string]::IsNullOrWhiteSpace($PublicLiteratureQueryPath)) {
    if (-not [IO.File]::Exists($PublicLiteratureQueryPath)) { throw "PublicLiteratureQueryPath does not exist: $PublicLiteratureQueryPath" }
    $cargoArguments += @("--mission-public-literature-query", $PublicLiteratureQueryPath)
}
if (-not [string]::IsNullOrWhiteSpace($PortfolioQueryPath)) {
    if (-not [IO.File]::Exists($PortfolioQueryPath)) { throw "PortfolioQueryPath does not exist: $PortfolioQueryPath" }
    $cargoArguments += @("--mission-portfolio-query", $PortfolioQueryPath)
}
if (-not [string]::IsNullOrWhiteSpace($FreshnessQueryPath)) {
    if (-not [IO.File]::Exists($FreshnessQueryPath)) { throw "FreshnessQueryPath does not exist: $FreshnessQueryPath" }
    $cargoArguments += @("--mission-freshness", $FreshnessQueryPath)
}
if (-not [string]::IsNullOrWhiteSpace($CaseAssetManifestPath)) {
    if (-not [IO.File]::Exists($CaseAssetManifestPath)) { throw "CaseAssetManifestPath does not exist: $CaseAssetManifestPath" }
    $cargoArguments += @("--case-asset-manifest", $CaseAssetManifestPath)
}
if (-not [string]::IsNullOrWhiteSpace($CaseAssetManifestQueryPath)) {
    if ([string]::IsNullOrWhiteSpace($CaseAssetManifestPath)) {
        throw "CaseAssetManifestQueryPath requires CaseAssetManifestPath"
    }
    if (-not [IO.File]::Exists($CaseAssetManifestQueryPath)) { throw "CaseAssetManifestQueryPath does not exist: $CaseAssetManifestQueryPath" }
    $cargoArguments += @("--case-asset-manifest-query", $CaseAssetManifestQueryPath)
}
if (-not [string]::IsNullOrWhiteSpace($MissionCaseAssetReviewDispositionPath)) {
    if (-not [IO.File]::Exists($MissionCaseAssetReviewDispositionPath)) { throw "MissionCaseAssetReviewDispositionPath does not exist: $MissionCaseAssetReviewDispositionPath" }
    $cargoArguments += @("--mission-case-asset-review-disposition", $MissionCaseAssetReviewDispositionPath)
}
if (-not [string]::IsNullOrWhiteSpace($CaseDicomImportPath)) {
    $cargoArguments += @("--mission-case-dicom", $CaseDicomImportPath)
}
if (-not [string]::IsNullOrWhiteSpace($CaseFhirImportPath)) {
    $cargoArguments += @("--mission-case-fhir", $CaseFhirImportPath)
}

$requestJson = Get-Content -LiteralPath $RequestPath -Raw
$requestDocument = $requestJson | ConvertFrom-Json
$missionJson = $requestJson | & cargo @cargoArguments
if ($LASTEXITCODE -ne 0) {
    throw "The offline neurosurgical mission exited with code $LASTEXITCODE"
}
$missionText = [string]::Join([Environment]::NewLine, @($missionJson))
$mission = ($missionText | ConvertFrom-Json)
if ($mission.provider -ne "none" -or $mission.network -ne $false -or $mission.human_review_required -ne $true) {
    throw "The neurosurgical mission violated the provider-free, network-free, human-review contract"
}
if ($null -eq $mission.evidence_synthesis -or
    $mission.evidence_synthesis.schema_version -ne "bioprism-neurosurgery-evidence-synthesis/0.1") {
    throw "The neurosurgical mission did not emit its digest-bound evidence synthesis ledger"
}
if ($mission.evidence_synthesis.provider -ne "none" -or
    $mission.evidence_synthesis.network -ne $false -or
    $mission.evidence_synthesis.human_review_required -ne $true) {
    throw "The evidence synthesis ledger violated the provider-free, network-free, human-review contract"
}
if ($null -eq $mission.real_data_coverage -or
    $mission.real_data_coverage.schema_version -ne "bioprism-neurosurgery-real-data-coverage/0.1" -or
    $mission.real_data_coverage.provenance_bound -ne $true -or
    $mission.real_data_coverage.synthetic_data -ne $false -or
    $mission.real_data_coverage.provider -ne "none" -or
    $mission.real_data_coverage.network -ne $false -or
    $mission.real_data_coverage.human_review_required -ne $true -or
    $mission.real_data_coverage.effect -ne "read_only") {
    throw "The mission did not emit a valid real-data coverage plane"
}
if ($null -eq $mission.real_data_trial_landscape -or
    $mission.real_data_trial_landscape.schema_version -ne "bioprism-neurosurgery-real-data-trial-landscape/0.1" -or
    $mission.real_data_trial_landscape.bundle_digest -ne $mission.real_data_coverage.bundle_digest -or
    $mission.real_data_trial_landscape.provenance_bound -ne $true -or
    $mission.real_data_trial_landscape.synthetic_data -ne $false -or
    $mission.real_data_trial_landscape.provider -ne "none" -or
    $mission.real_data_trial_landscape.network -ne $false -or
    $mission.real_data_trial_landscape.human_review_required -ne $true -or
    $mission.real_data_trial_landscape.effect -ne "read_only") {
    throw "The mission did not emit a digest-bound real-data trial landscape"
}
if ($null -eq $mission.real_data_molecular_coverage -or
    $mission.real_data_molecular_coverage.schema_version -ne "bioprism-neurosurgery-real-data-molecular-coverage/0.1" -or
    $mission.real_data_molecular_coverage.bundle_digest -ne $mission.real_data_coverage.bundle_digest -or
    $mission.real_data_molecular_coverage.provenance_bound -ne $true -or
    $mission.real_data_molecular_coverage.synthetic_data -ne $false -or
    $mission.real_data_molecular_coverage.provider -ne "none" -or
    $mission.real_data_molecular_coverage.network -ne $false -or
    $mission.real_data_molecular_coverage.human_review_required -ne $true -or
    $mission.real_data_molecular_coverage.effect -ne "read_only") {
    throw "The mission did not emit a digest-bound real-data molecular coverage ledger"
}
if ($null -eq $mission.real_data_cohort_landscape -or
    $mission.real_data_cohort_landscape.schema_version -ne "bioprism-neurosurgery-real-data-cohort-landscape/0.1" -or
    $mission.real_data_cohort_landscape.bundle_digest -ne $mission.real_data_coverage.bundle_digest -or
    $mission.real_data_cohort_landscape.provenance_bound -ne $true -or
    $mission.real_data_cohort_landscape.synthetic_data -ne $false -or
    $mission.real_data_cohort_landscape.provider -ne "none" -or
    $mission.real_data_cohort_landscape.network -ne $false -or
    $mission.real_data_cohort_landscape.human_review_required -ne $true -or
    $mission.real_data_cohort_landscape.effect -ne "read_only" -or
    $mission.real_data_cohort_landscape.returned_project_count -lt 1) {
    throw "The mission did not emit a digest-bound real-data cohort landscape"
}
$hasCaseDicom = -not [string]::IsNullOrWhiteSpace($CaseDicomImportPath)
$hasCaseFhir = -not [string]::IsNullOrWhiteSpace($CaseFhirImportPath)
if ($hasCaseDicom -or $hasCaseFhir) {
    if ($null -eq $mission.case_asset_manifest -or
        $mission.evidence_synthesis.case_asset_summary.report_digest -ne $mission.case_asset_manifest.report_digest) {
        throw "The mission case-import receipts did not remain bound to the digest-only asset projection"
    }
    $manifestAssetRefs = @($mission.case_asset_manifest.assets | ForEach-Object { [string]$_.asset_ref })
    if ($hasCaseDicom) {
        $dicom = $mission.case_dicom_import
        $dicomBound = $null -ne $dicom -and
            $dicom.schema_version -eq "bioprism-neurosurgery-case-dicom-import/0.1" -and
            $dicom.deidentified -eq $true -and
            $dicom.synthetic_data -eq $false -and
            $dicom.provider -eq "none" -and
            $dicom.network -eq $false -and
            $dicom.human_review_required -eq $true -and
            $dicom.manifest_report.request_digest -eq $mission.case_asset_manifest.request_digest
        if ($dicomBound) {
            foreach ($asset in @($dicom.manifest_report.assets)) {
                if ($manifestAssetRefs -notcontains ([string]$asset.asset_ref)) {
                    $dicomBound = $false
                    break
                }
            }
        }
        if (-not $dicomBound) {
            throw "The mission DICOM receipt did not remain bound to the composed asset projection"
        }
    }
    if ($hasCaseFhir) {
        $fhir = $mission.case_fhir_import
        $fhirBound = $null -ne $fhir -and
            $fhir.schema_version -eq "bioprism-neurosurgery-case-fhir-import/0.1" -and
            $fhir.deidentified -eq $true -and
            $fhir.synthetic_data -eq $false -and
            $fhir.provider -eq "none" -and
            $fhir.network -eq $false -and
            $fhir.human_review_required -eq $true -and
            $fhir.manifest_report.request_digest -eq $mission.case_asset_manifest.request_digest
        if ($fhirBound) {
            foreach ($asset in @($fhir.manifest_report.assets)) {
                if ($manifestAssetRefs -notcontains ([string]$asset.asset_ref)) {
                    $fhirBound = $false
                    break
                }
            }
        }
        if (-not $fhirBound) {
            throw "The mission FHIR receipt did not remain bound to the composed asset projection"
        }
    }
}
if ($null -ne $mission.real_data_evidence_packet) {
    $molecularCoverage = $mission.real_data_evidence_packet.molecular_coverage
    if ($null -eq $molecularCoverage -or
        $molecularCoverage.schema_version -ne "bioprism-neurosurgery-real-data-molecular-coverage/0.1" -or
        $molecularCoverage.synthetic_data -ne $false -or
        $molecularCoverage.provider -ne "none" -or
        $molecularCoverage.network -ne $false -or
        $molecularCoverage.human_review_required -ne $true -or
        $molecularCoverage.missing_alteration_type_count -lt 0 -or
        $molecularCoverage.missing_datatype_count -lt 0) {
        throw "The molecular coverage ledger violated the real-data, provider-free, network-free, human-review contract"
    }
    $reconciliation = $mission.real_data_evidence_packet.reconciliation
    if ($null -eq $reconciliation -or
        $reconciliation.schema_version -ne "bioprism-neurosurgery-real-data-reconciliation/0.1" -or
        $reconciliation.bundle_digest -ne $mission.real_data_coverage.bundle_digest -or
        $reconciliation.provenance_bound -ne $true -or
        $reconciliation.synthetic_data -ne $false -or
        $reconciliation.provider -ne "none" -or
        $reconciliation.network -ne $false -or
        $reconciliation.human_review_required -ne $true -or
        $reconciliation.effect -ne "read_only" -or
        $reconciliation.candidate_issue_count -lt 0 -or
        $reconciliation.returned_issue_count -lt 0 -or
        $reconciliation.omitted_issue_count -lt 0 -or
        $reconciliation.returned_issue_count -gt $reconciliation.candidate_issue_count -or
        $reconciliation.omitted_issue_count -ne ($reconciliation.candidate_issue_count - $reconciliation.returned_issue_count) -or
        $reconciliation.truncated -ne ($reconciliation.omitted_issue_count -gt 0) -or
        $reconciliation.requires_review -ne ($reconciliation.candidate_issue_count -gt 0)) {
        throw "The identifier reconciliation ledger violated the real-data, provider-free, network-free, human-review contract"
    }
}
$publicIntegrity = $mission.public_literature_integrity_audit
$publicPacket = $mission.public_literature_evidence_packet
$publicContext = $mission.public_literature_reasoning_context
$publicWorkbench = $mission.public_literature_workbench
if ($null -eq $publicIntegrity -or
    $publicIntegrity.schema_version -ne "bioprism-neurosurgery-public-literature-integrity-audit/0.1" -or
    $publicIntegrity.provider -ne "none" -or
    $publicIntegrity.network -ne $false -or
    $publicIntegrity.synthetic_data -ne $false -or
    $publicIntegrity.provenance_bound -ne $true -or
    $publicIntegrity.human_review_required -ne $true -or
    $null -eq $publicPacket -or
    $publicPacket.schema_version -ne "bioprism-neurosurgery-public-literature-evidence-packet/0.1" -or
    $publicPacket.bundle_digest -ne $publicIntegrity.bundle_digest -or
    $publicPacket.provider -ne "none" -or
    $publicPacket.network -ne $false -or
    $publicPacket.synthetic_data -ne $false -or
    $publicPacket.provenance_bound -ne $true -or
    $publicPacket.human_review_required -ne $true -or
    $publicPacket.effect -ne "read_only" -or
    $null -eq $publicContext -or
    $publicContext.schema_version -ne "bioprism-neurosurgery-public-literature-reasoning-context/0.1" -or
    $publicContext.bundle_digest -ne $publicIntegrity.bundle_digest -or
    $publicContext.provider -ne "none" -or
    $publicContext.network -ne $false -or
    $publicContext.synthetic_data -ne $false -or
    $publicContext.provenance_bound -ne $true -or
    $publicContext.human_review_required -ne $true -or
    $publicContext.effect -ne "read_only" -or
    $null -eq $publicWorkbench -or
    $publicWorkbench.schema_version -ne "bioprism-neurosurgery-public-literature-workbench/0.1" -or
    $publicWorkbench.bundle_digest -ne $publicIntegrity.bundle_digest -or
    $publicWorkbench.provider -ne "none" -or
    $publicWorkbench.network -ne $false -or
    $publicWorkbench.synthetic_data -ne $false -or
    $publicWorkbench.provenance_bound -ne $true -or
    $publicWorkbench.human_review_required -ne $true -or
    $publicWorkbench.effect -ne "read_only") {
    throw "The public-literature evidence plane violated the real-data, provider-free, network-free, human-review contract"
}
if (-not [string]::IsNullOrWhiteSpace($CaseAssetManifestPath)) {
    if ($null -eq $mission.case_asset_manifest -or
        $mission.case_asset_manifest.schema_version -ne "bioprism-neurosurgery-case-asset-manifest/0.1" -or
        $mission.case_asset_manifest.synthetic_data -ne $false -or
        $mission.case_asset_manifest.provider -ne "none" -or
        $mission.case_asset_manifest.network -ne $false -or
        $mission.case_asset_manifest.human_review_required -ne $true) {
        throw "The case-asset manifest violated the real-data, provider-free, network-free, human-review contract"
    }
    $assetSummary = $mission.evidence_synthesis.case_asset_summary
    if ($null -eq $assetSummary -or
        $assetSummary.report_digest -ne $mission.case_asset_manifest.report_digest -or
        $assetSummary.asset_count -ne $mission.case_asset_manifest.asset_count -or
        $assetSummary.observed_asset_count -ne $mission.case_asset_manifest.observed_asset_count -or
        $assetSummary.non_observed_asset_count -ne $mission.case_asset_manifest.non_observed_asset_count -or
        $assetSummary.provenance_complete_asset_count -ne $mission.case_asset_manifest.provenance_complete_asset_count) {
        throw "The evidence synthesis asset summary did not remain digest-bound to the case-asset projection"
    }
}
if (-not [string]::IsNullOrWhiteSpace($MissionCaseAssetReviewDispositionPath)) {
    if ($null -eq $mission.case_asset_review_disposition -or
        $mission.case_asset_review_disposition.report_digest -ne $mission.case_asset_manifest.report_digest -or
        $mission.evidence_synthesis.case_asset_review_disposition_digest -ne $mission.case_asset_review_disposition.disposition_digest -or
        $mission.mission_audit.integrity_ok -ne $true) {
        throw "The mission case-asset disposition did not remain bound to the manifest, synthesis, and mission audit"
    }
}
if ($requestDocument.specialty -eq "glioma" -and $null -ne $requestDocument.glioma_molecular) {
    $molecularMap = $mission.evidence_synthesis.glioma_molecular_map
    if ($null -eq $molecularMap -or
        $molecularMap.schema_version -ne "bioprism-neurosurgery-glioma-molecular-map/0.1" -or
        $molecularMap.provider -ne "none" -or
        $molecularMap.network -ne $false -or
        $molecularMap.human_review_required -ne $true) {
        throw "The typed glioma request did not receive its provider-free, human-review molecular evidence map"
    }
}

# Persist and replay the exact emitted envelope before returning it. This makes the unattended
# runner restart-safe: a caller receives a file that has already passed the same request/snapshot
# replay gate exposed by the CLI and MCP. The temporary file is moved into place only after the
# complete mission has been serialized.
$missionOutputFullPath = [IO.Path]::GetFullPath($MissionOutputPath)
$missionOutputParent = Split-Path -Parent $missionOutputFullPath
if (-not [string]::IsNullOrWhiteSpace($missionOutputParent) -and
    -not [IO.Directory]::Exists($missionOutputParent)) {
    [void][IO.Directory]::CreateDirectory($missionOutputParent)
}
$missionTempPath = "$missionOutputFullPath.tmp.$PID"
try {
    [IO.File]::WriteAllText($missionTempPath, $missionText, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $missionTempPath -Destination $missionOutputFullPath -Force
}
catch {
    if ([IO.File]::Exists($missionTempPath)) {
        Remove-Item -LiteralPath $missionTempPath -Force -ErrorAction SilentlyContinue
    }
    throw "Could not persist mission output ${missionOutputFullPath}: $($_.Exception.Message)"
}

$validationArguments = @(
    "run",
    "-p", "bioprism-neurosurgery",
    "--offline",
    "--quiet",
    "--",
    "--validate-mission", $missionOutputFullPath,
    "--real-glioma", $RealDataPath,
    "--public-literature", $PublicLiteraturePath
)
if (-not [string]::IsNullOrWhiteSpace($CaseDicomImportPath)) {
    $validationArguments += @("--mission-case-dicom", $CaseDicomImportPath)
}
if (-not [string]::IsNullOrWhiteSpace($CaseFhirImportPath)) {
    $validationArguments += @("--mission-case-fhir", $CaseFhirImportPath)
}
$validationJson = $requestJson | & cargo @validationArguments
if ($LASTEXITCODE -ne 0) {
    throw "Persisted neurosurgical mission replay exited with code $LASTEXITCODE"
}
$validationText = [string]::Join([Environment]::NewLine, @($validationJson))
$validation = ($validationText | ConvertFrom-Json)
if ($validation.valid -ne $true -or
    $validation.mission_id -ne $mission.mission_id -or
    $validation.provider -ne "none" -or
    $validation.network -ne $false) {
    throw "Persisted neurosurgical mission replay did not pass the provider-free exact-input gate"
}
# Emit only the mission envelope so callers can pipe this command directly to ConvertFrom-Json.
$missionText
