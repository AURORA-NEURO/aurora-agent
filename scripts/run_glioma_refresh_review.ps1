param(
    [string]$SnapshotPath = "data/neurosurgery/glioma_public_snapshot.json",
    [string]$RequestPath = "data/neurosurgery/glioma_real_request.json",
    [string]$CandidatePath = "work/glioma_public_snapshot.candidate.json",
    [string]$ReportPath = "work/glioma_refresh_audit.json",
    [int]$PubMedLimit = 20,
    [string[]]$GdcProjectIds = @("TCGA-GBM"),
    [string]$PubMedTerm = "glioblastoma AND (molecular OR genomic)",
    [string]$PubMedSourceId = "pubmed_glioblastoma",
    [int]$TimeoutSec = 30
)

$ErrorActionPreference = "Stop"

function Resolve-ExistingPath([string]$Path, [string]$Field) {
    $resolved = [IO.Path]::GetFullPath($Path)
    if (-not [IO.File]::Exists($resolved)) {
        throw "$Field does not exist: $resolved"
    }
    return $resolved
}

$snapshotFullPath = Resolve-ExistingPath $SnapshotPath "SnapshotPath"
$requestFullPath = Resolve-ExistingPath $RequestPath "RequestPath"
$candidateFullPath = [IO.Path]::GetFullPath($CandidatePath)
$reportFullPath = [IO.Path]::GetFullPath($ReportPath)
if ($candidateFullPath -eq $snapshotFullPath) {
    throw "CandidatePath must differ from SnapshotPath; the review workflow never overwrites the source snapshot"
}
if ($PubMedLimit -lt 1 -or $PubMedLimit -gt 50) {
    throw "PubMedLimit must be between 1 and 50"
}
if ($TimeoutSec -lt 1 -or $TimeoutSec -gt 120) {
    throw "TimeoutSec must be between 1 and 120"
}

$candidateDirectory = [IO.Path]::GetDirectoryName($candidateFullPath)
$reportDirectory = [IO.Path]::GetDirectoryName($reportFullPath)
New-Item -ItemType Directory -Force -Path $candidateDirectory | Out-Null
New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$refreshScript = Join-Path $scriptRoot "refresh_glioma_public_data.ps1"
$requestText = [IO.File]::ReadAllText($requestFullPath)
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

try {
    # Validate the baseline before any network work. This catches accidental edits and keeps the
    # before digest authoritative for the subsequent reconciliation report.
    $baseline = cargo run -p bioprism-neurosurgery --offline --quiet -- --validate-real-glioma $snapshotFullPath
    if ($LASTEXITCODE -ne 0) {
        throw "Rust validation rejected the baseline snapshot: $baseline"
    }

    $refreshParameters = @{
        OutputPath = $candidateFullPath
        PubMedLimit = $PubMedLimit
        GdcProjectIds = $GdcProjectIds
        PubMedTerm = $PubMedTerm
        PubMedSourceId = $PubMedSourceId
        TimeoutSec = $TimeoutSec
    }
    & $refreshScript @refreshParameters | Out-Null
    if (-not [IO.File]::Exists($candidateFullPath)) {
        throw "refresh script did not produce a candidate snapshot"
    }

    # The core receives both real snapshots and emits a deterministic report. It does not fetch,
    # merge, or promote the candidate; a reviewer owns the final disposition.
    $auditJson = $requestText | cargo run -p bioprism-neurosurgery --offline --quiet -- --real-data-refresh-audit $snapshotFullPath $candidateFullPath
    if ($LASTEXITCODE -ne 0) {
        throw "Rust refresh audit failed: $auditJson"
    }
    $audit = $auditJson | ConvertFrom-Json
    if ($audit.synthetic_data -ne $false -or $audit.provider -ne "none" -or $audit.network -ne $false) {
        throw "refresh audit did not preserve the provider-free real-data contract"
    }
    [IO.File]::WriteAllText($reportFullPath, $auditJson, $utf8NoBom)
    [pscustomobject][ordered]@{
        candidate_path = $candidateFullPath
        audit_path = $reportFullPath
        before_bundle_digest = [string]$audit.before_bundle_digest
        after_bundle_digest = [string]$audit.after_bundle_digest
        structural_change_detected = [bool]$audit.structural_change_detected
        requires_refresh_review = [bool]$audit.requires_refresh_review
        source_identity_stable = [bool]$audit.source_identity_stable
        record_identity_stable = [bool]$audit.record_identity_stable
        human_review_required = [bool]$audit.human_review_required
        promotion = "not_performed; reviewer must inspect audit and explicitly promote"
    } | ConvertTo-Json -Compress
}
catch {
    Remove-Item -LiteralPath $candidateFullPath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $reportFullPath -Force -ErrorAction SilentlyContinue
    throw
}
