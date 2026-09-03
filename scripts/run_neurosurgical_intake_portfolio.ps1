param(
    [Parameter(Mandatory = $true)]
    [string]$QueryPath,
    [string]$RealDataPath = "data/neurosurgery/glioma_public_snapshot.json",
    [string]$PublicLiteraturePath = "data/neurosurgery/neurosurgical_public_literature_snapshot.json",
    [string]$RealRefreshRequestPath = "data/neurosurgery/glioma_real_request.json",
    [string]$RealCandidatePath = "work/glioma_public_snapshot.intake-candidate.json",
    [string]$PublicCandidatePath = "work/neurosurgical_public_literature_snapshot.intake-candidate.json",
    [string]$RealAuditPath = "work/glioma_intake_refresh_audit.json",
    [string]$PublicAuditPath = "work/neurosurgical_public_literature_intake_refresh_audit.json",
    [int]$PubMedLimit = 20,
    [string[]]$GdcProjectIds = @("TCGA-GBM"),
    [string]$PubMedTerm = "glioblastoma AND (molecular OR genomic)",
    [string]$PubMedSourceId = "pubmed_glioblastoma",
    [int]$PerSpecialtyLimit = 10,
    [int]$TimeoutSec = 60,
    [string]$FreshnessQueryPath,
    [string]$CaseAssetManifestPath,
    [string]$CaseAssetManifestQueryPath,
    [string]$CaseAssetReviewDispositionPath,
    [switch]$SkipRefresh
)

$ErrorActionPreference = "Stop"

function Resolve-ExistingPath([string]$Path, [string]$Field) {
    $resolved = [IO.Path]::GetFullPath($Path)
    if (-not [IO.File]::Exists($resolved)) {
        throw "$Field does not exist: $resolved"
    }
    return $resolved
}

function Resolve-OutputPath([string]$Path) {
    return [IO.Path]::GetFullPath($Path)
}

function Read-JsonFile([string]$Path, [string]$Field) {
    $text = [IO.File]::ReadAllText($Path)
    try {
        return ($text | ConvertFrom-Json)
    }
    catch {
        throw "$Field is not valid JSON: $($_.Exception.Message)"
    }
}

if ($PubMedLimit -lt 1 -or $PubMedLimit -gt 50) {
    throw "PubMedLimit must be between 1 and 50"
}
if ($PerSpecialtyLimit -lt 1 -or $PerSpecialtyLimit -gt 50) {
    throw "PerSpecialtyLimit must be between 1 and 50"
}
if ($TimeoutSec -lt 1 -or $TimeoutSec -gt 120) {
    throw "TimeoutSec must be between 1 and 120"
}

$queryFullPath = Resolve-ExistingPath $QueryPath "QueryPath"
$realFullPath = Resolve-ExistingPath $RealDataPath "RealDataPath"
$publicFullPath = Resolve-ExistingPath $PublicLiteraturePath "PublicLiteraturePath"
$refreshRequestFullPath = Resolve-ExistingPath $RealRefreshRequestPath "RealRefreshRequestPath"
$freshnessFullPath = $null
if (-not [string]::IsNullOrWhiteSpace($FreshnessQueryPath)) {
    $freshnessFullPath = Resolve-ExistingPath $FreshnessQueryPath "FreshnessQueryPath"
    $null = Read-JsonFile $freshnessFullPath "FreshnessQueryPath"
}
$caseAssetManifestFullPath = $null
if (-not [string]::IsNullOrWhiteSpace($CaseAssetManifestPath)) {
    $caseAssetManifestFullPath = Resolve-ExistingPath $CaseAssetManifestPath "CaseAssetManifestPath"
    $null = Read-JsonFile $caseAssetManifestFullPath "CaseAssetManifestPath"
}
$caseAssetManifestQueryFullPath = $null
if (-not [string]::IsNullOrWhiteSpace($CaseAssetManifestQueryPath)) {
    if ($null -eq $caseAssetManifestFullPath) {
        throw "CaseAssetManifestQueryPath requires CaseAssetManifestPath"
    }
    $caseAssetManifestQueryFullPath = Resolve-ExistingPath $CaseAssetManifestQueryPath "CaseAssetManifestQueryPath"
    $null = Read-JsonFile $caseAssetManifestQueryFullPath "CaseAssetManifestQueryPath"
}
$caseAssetReviewDispositionFullPath = $null
if (-not [string]::IsNullOrWhiteSpace($CaseAssetReviewDispositionPath)) {
    if ($null -eq $caseAssetManifestFullPath) {
        throw "CaseAssetReviewDispositionPath requires CaseAssetManifestPath"
    }
    $caseAssetReviewDispositionFullPath = Resolve-ExistingPath $CaseAssetReviewDispositionPath "CaseAssetReviewDispositionPath"
    $null = Read-JsonFile $caseAssetReviewDispositionFullPath "CaseAssetReviewDispositionPath"
}
$queryText = [IO.File]::ReadAllText($queryFullPath)
$query = Read-JsonFile $queryFullPath "QueryPath"
if ($null -eq $query.question -or [string]::IsNullOrWhiteSpace([string]$query.question)) {
    throw "QueryPath must contain a non-empty question"
}

$realCandidateFullPath = Resolve-OutputPath $RealCandidatePath
$publicCandidateFullPath = Resolve-OutputPath $PublicCandidatePath
$realAuditFullPath = Resolve-OutputPath $RealAuditPath
$publicAuditFullPath = Resolve-OutputPath $PublicAuditPath
if ([string]::Equals($realCandidateFullPath, $realFullPath, [StringComparison]::OrdinalIgnoreCase)) {
    throw "RealCandidatePath must differ from RealDataPath"
}
if ([string]::Equals($publicCandidateFullPath, $publicFullPath, [StringComparison]::OrdinalIgnoreCase)) {
    throw "PublicCandidatePath must differ from PublicLiteraturePath"
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$realReviewScript = Join-Path $scriptRoot "run_glioma_refresh_review.ps1"
$publicReviewScript = Join-Path $scriptRoot "run_neurosurgical_public_literature_refresh_review.ps1"
$realCandidateCreated = $false
$publicCandidateCreated = $false
$realAudit = $null
$publicAudit = $null

try {
    if (-not $SkipRefresh) {
        if ([IO.File]::Exists($realCandidateFullPath) -or [IO.File]::Exists($publicCandidateFullPath)) {
            throw "candidate paths already exist; choose fresh paths so stale snapshots cannot enter the worker"
        }
        New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($realCandidateFullPath)) | Out-Null
        New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($publicCandidateFullPath)) | Out-Null
        New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($realAuditFullPath)) | Out-Null
        New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($publicAuditFullPath)) | Out-Null

        # Each review wrapper validates the last-known-good baseline, refreshes only public
        # endpoints into a separate candidate, hashes and validates it offline, and emits a
        # refresh audit. Neither wrapper promotes the candidate.
        & $realReviewScript `
            -SnapshotPath $realFullPath `
            -RequestPath $refreshRequestFullPath `
            -CandidatePath $realCandidateFullPath `
            -ReportPath $realAuditFullPath `
            -PubMedLimit $PubMedLimit `
            -GdcProjectIds $GdcProjectIds `
            -PubMedTerm $PubMedTerm `
            -PubMedSourceId $PubMedSourceId `
            -TimeoutSec $TimeoutSec | Out-Null
        if ($LASTEXITCODE -ne 0 -or -not [IO.File]::Exists($realCandidateFullPath)) {
            throw "real glioma refresh review did not produce a validated candidate"
        }
        $realCandidateCreated = $true

        & $publicReviewScript `
            -SnapshotPath $publicFullPath `
            -CandidatePath $publicCandidateFullPath `
            -ReportPath $publicAuditFullPath `
            -PerSpecialtyLimit $PerSpecialtyLimit `
            -TimeoutSec $TimeoutSec | Out-Null
        if ($LASTEXITCODE -ne 0 -or -not [IO.File]::Exists($publicCandidateFullPath)) {
            throw "public-literature refresh review did not produce a validated candidate"
        }
        $publicCandidateCreated = $true
        $realAudit = Read-JsonFile $realAuditFullPath "RealAuditPath"
        $publicAudit = Read-JsonFile $publicAuditFullPath "PublicAuditPath"
    }

    $runRealPath = if ($SkipRefresh) { $realFullPath } else { $realCandidateFullPath }
    $runPublicPath = if ($SkipRefresh) { $publicFullPath } else { $publicCandidateFullPath }
    $cargoArguments = @(
        "run", "-p", "bioprism-neurosurgery", "--offline", "--quiet", "--",
        "--intake-portfolio", "--public-literature", $runPublicPath
    )
    if ($null -ne $freshnessFullPath) {
        $cargoArguments += @("--intake-freshness", $freshnessFullPath)
    }
    if ($null -ne $caseAssetManifestFullPath) {
        $cargoArguments += @("--case-asset-manifest", $caseAssetManifestFullPath)
    }
    if ($null -ne $caseAssetManifestQueryFullPath) {
        $cargoArguments += @("--case-asset-manifest-query", $caseAssetManifestQueryFullPath)
    }
    if ($null -ne $caseAssetReviewDispositionFullPath) {
        $cargoArguments += @("--intake-case-asset-review-disposition", $caseAssetReviewDispositionFullPath)
    }

    # Start with the citation plane. If lexical intake or an explicit all-lane scope says that
    # glioma population evidence is required, rerun with the validated real bundle as well. This
    # keeps non-glioma requests from being rejected merely because the worker has a glioma file.
    $portfolioJson = $queryText | & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "intake portfolio exited with code ${LASTEXITCODE}: $portfolioJson"
    }
    $portfolio = ([string]::Join([Environment]::NewLine, @($portfolioJson)) | ConvertFrom-Json)
    $requiresReal = $false
    if ($portfolio.required_evidence -and @($portfolio.required_evidence) -contains "real_glioma_snapshot") {
        $requiresReal = $true
    }
    if ($requiresReal) {
        $cargoArguments += @("--real-glioma", $runRealPath)
        $portfolioJson = $queryText | & cargo @cargoArguments
        if ($LASTEXITCODE -ne 0) {
            throw "intake portfolio with real glioma evidence exited with code ${LASTEXITCODE}: $portfolioJson"
        }
        $portfolio = ([string]::Join([Environment]::NewLine, @($portfolioJson)) | ConvertFrom-Json)
    }

    if ($portfolio.provider -ne "none" -or $portfolio.network -ne $false -or $portfolio.human_review_required -ne $true) {
        throw "intake portfolio violated the provider-free human-review contract"
    }
    if ($null -ne $portfolio.portfolio -and $portfolio.portfolio.synthetic_data -ne $false) {
        throw "intake portfolio did not preserve synthetic_data=false"
    }
    if ($null -ne $caseAssetManifestFullPath) {
        $selectedMission = $portfolio.mission
        if ($null -ne $selectedMission -and
            ($null -eq $selectedMission.case_asset_manifest -or
            $null -eq $selectedMission.evidence_synthesis.case_asset_summary -or
            $selectedMission.evidence_synthesis.case_asset_summary.report_digest -ne $selectedMission.case_asset_manifest.report_digest -or
            $selectedMission.evidence_synthesis.case_asset_summary.asset_count -ne $selectedMission.case_asset_manifest.asset_count -or
            $selectedMission.evidence_synthesis.case_asset_summary.observed_asset_count -ne $selectedMission.case_asset_manifest.observed_asset_count -or
            $selectedMission.evidence_synthesis.case_asset_summary.non_observed_asset_count -ne $selectedMission.case_asset_manifest.non_observed_asset_count -or
            $selectedMission.evidence_synthesis.case_asset_summary.provenance_complete_asset_count -ne $selectedMission.case_asset_manifest.provenance_complete_asset_count)) {
            throw "selected intake mission did not preserve the digest-bound case-asset summary"
        }
        if ($null -ne $caseAssetReviewDispositionFullPath -and $null -ne $selectedMission) {
            $disposition = Read-JsonFile $caseAssetReviewDispositionFullPath "CaseAssetReviewDispositionPath"
            if ($null -eq $selectedMission.case_asset_review_disposition -or
                $selectedMission.case_asset_review_disposition.disposition_digest -ne $disposition.disposition_digest) {
                throw "selected intake mission did not preserve the digest-bound case-asset disposition"
            }
        }
    }
    $executionSnapshotPaths = @($runPublicPath)
    if ($requiresReal) {
        $executionSnapshotPaths += $runRealPath
    }

    [pscustomobject][ordered]@{
        schema = "bioprism-neurosurgery-autonomous-intake-worker/0.1"
        query_path = $queryFullPath
        source_snapshot_paths = @($realFullPath, $publicFullPath)
        execution_snapshot_paths = $executionSnapshotPaths
        freshness_query_path = $freshnessFullPath
        case_asset_manifest_path = $caseAssetManifestFullPath
        case_asset_manifest_query_path = $caseAssetManifestQueryFullPath
        case_asset_review_disposition_path = $caseAssetReviewDispositionFullPath
        refreshed = (-not $SkipRefresh)
        real_refresh_audit = $realAudit
        public_refresh_audit = $publicAudit
        portfolio = $portfolio
        status = [string]$portfolio.status
        provider = [string]$portfolio.provider
        network = [bool]$portfolio.network
        human_review_required = [bool]$portfolio.human_review_required
        promotion = "not_performed; candidate snapshots and refresh audits remain for explicit human review"
    } | ConvertTo-Json -Depth 80
}
catch {
    if ($realCandidateCreated -and [IO.File]::Exists($realCandidateFullPath)) {
        Remove-Item -LiteralPath $realCandidateFullPath -Force -ErrorAction SilentlyContinue
    }
    if ($publicCandidateCreated -and [IO.File]::Exists($publicCandidateFullPath)) {
        Remove-Item -LiteralPath $publicCandidateFullPath -Force -ErrorAction SilentlyContinue
    }
    throw
}
