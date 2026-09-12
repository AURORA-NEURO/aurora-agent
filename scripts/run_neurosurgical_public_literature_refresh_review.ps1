param(
    [string]$SnapshotPath = "data/neurosurgery/neurosurgical_public_literature_snapshot.json",
    [string]$CandidatePath = "work/neurosurgical_public_literature_snapshot.candidate.json",
    [string]$ReportPath = "work/neurosurgical_public_literature_refresh_audit.json",
    [string]$AuditQueryPath = "",
    [int]$PerSpecialtyLimit = 10,
    [int]$TimeoutSec = 60
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
$candidateFullPath = [IO.Path]::GetFullPath($CandidatePath)
$reportFullPath = [IO.Path]::GetFullPath($ReportPath)
if ([string]::Equals($candidateFullPath, $snapshotFullPath, [StringComparison]::OrdinalIgnoreCase)) {
    throw "CandidatePath must differ from SnapshotPath; the review workflow never overwrites the source snapshot"
}
if ([IO.File]::Exists($candidateFullPath)) {
    throw "CandidatePath already exists; choose a new candidate path so stale data cannot be mistaken for this refresh"
}
if ($PerSpecialtyLimit -lt 1 -or $PerSpecialtyLimit -gt 50) {
    throw "PerSpecialtyLimit must be between 1 and 50"
}
if ($TimeoutSec -lt 1 -or $TimeoutSec -gt 120) {
    throw "TimeoutSec must be between 1 and 120"
}

$auditQueryText = "{}"
if (-not [string]::IsNullOrWhiteSpace($AuditQueryPath)) {
    $auditQueryFullPath = Resolve-ExistingPath $AuditQueryPath "AuditQueryPath"
    $auditQueryText = [IO.File]::ReadAllText($auditQueryFullPath)
}

$candidateDirectory = [IO.Path]::GetDirectoryName($candidateFullPath)
$reportDirectory = [IO.Path]::GetDirectoryName($reportFullPath)
New-Item -ItemType Directory -Force -Path $candidateDirectory | Out-Null
New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$refreshScript = Join-Path $scriptRoot "refresh_neurosurgical_public_literature.ps1"
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$candidateCreated = $false

try {
    # Validate the baseline before any network work. This keeps the before digest authoritative
    # and prevents an invalid local snapshot from being used as a refresh comparison anchor.
    $baseline = cargo run -p bioprism-neurosurgery --offline --quiet -- --validate-public-literature $snapshotFullPath
    if ($LASTEXITCODE -ne 0) {
        throw "Rust validation rejected the baseline public-literature snapshot: $baseline"
    }

    & $refreshScript -OutputPath $candidateFullPath -PerSpecialtyLimit $PerSpecialtyLimit -TimeoutSec $TimeoutSec | Out-Null
    if (-not [IO.File]::Exists($candidateFullPath)) {
        throw "refresh script did not produce a candidate snapshot"
    }
    $candidateCreated = $true

    # The Rust core receives two already validated snapshots and emits a deterministic audit. It
    # never fetches, merges, accepts, or promotes the candidate; a reviewer owns promotion.
    $auditJson = $auditQueryText | cargo run -p bioprism-neurosurgery --offline --quiet -- --public-literature-refresh-audit $snapshotFullPath $candidateFullPath
    if ($LASTEXITCODE -ne 0) {
        throw "Rust public-literature refresh audit failed: $auditJson"
    }
    $auditJsonText = [string]::Join([Environment]::NewLine, @($auditJson))
    $audit = $auditJsonText | ConvertFrom-Json
    if ($audit.synthetic_data -ne $false -or $audit.provider -ne "none" -or $audit.network -ne $false) {
        throw "refresh audit did not preserve the provider-free real-data contract"
    }
    if ($audit.human_review_required -ne $true) {
        throw "refresh audit did not preserve the mandatory human-review boundary"
    }
    [IO.File]::WriteAllText($reportFullPath, $auditJsonText, $utf8NoBom)
    [pscustomobject][ordered]@{
        candidate_path = $candidateFullPath
        audit_path = $reportFullPath
        before_bundle_digest = [string]$audit.before_bundle_digest
        after_bundle_digest = [string]$audit.after_bundle_digest
        structural_change_detected = [bool]$audit.structural_change_detected
        specialty_coverage_changed = [bool]$audit.specialty_coverage_changed
        requires_refresh_review = [bool]$audit.requires_refresh_review
        source_identity_stable = [bool]$audit.source_identity_stable
        record_identity_stable = [bool]$audit.record_identity_stable
        human_review_required = [bool]$audit.human_review_required
        promotion = "not_performed; reviewer must inspect the audit and explicitly promote the candidate"
    } | ConvertTo-Json -Compress
}
catch {
    if ($candidateCreated -and [IO.File]::Exists($candidateFullPath)) {
        Remove-Item -LiteralPath $candidateFullPath -Force -ErrorAction SilentlyContinue
    }
    throw
}
