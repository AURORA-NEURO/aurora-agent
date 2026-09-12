param(
    [string]$RequestPath = "data/neurosurgery/glioma_real_request.json",
    [string]$RealGliomaPath = "data/neurosurgery/glioma_public_snapshot.json",
    [string]$PublicLiteraturePath = "data/neurosurgery/neurosurgical_public_literature_snapshot.json",
    [string]$QueryPath = "",
    [string]$CaseAssetManifestPath = "",
    [string]$CaseAssetManifestQueryPath = "",
    [string]$CaseAssetReviewDispositionPath = "",
    [string]$SessionPath = "work/neurosurgical-acquisition-session.json",
    [string]$StartOutputPath = "work/neurosurgical-acquisition-start.json",
    [string]$AdvanceOutputPath = "work/neurosurgical-acquisition-advance.json",
    [string]$FinishOutputPath = "work/neurosurgical-acquisition-finish.json",
    [int]$MaxAdvanceSteps = 4,
    [int]$MaxWaves = 64
)

$ErrorActionPreference = "Stop"

if ($MaxAdvanceSteps -lt 1 -or $MaxAdvanceSteps -gt 16) {
    throw "MaxAdvanceSteps must be between 1 and 16"
}
if ($MaxWaves -lt 1 -or $MaxWaves -gt 64) {
    throw "MaxWaves must be between 1 and 64"
}
if (-not (Test-Path -LiteralPath $RequestPath)) { throw "RequestPath does not exist: $RequestPath" }
if (-not (Test-Path -LiteralPath $RealGliomaPath)) { throw "RealGliomaPath does not exist: $RealGliomaPath" }
if (-not (Test-Path -LiteralPath $PublicLiteraturePath)) { throw "PublicLiteraturePath does not exist: $PublicLiteraturePath" }
if ($QueryPath -and -not (Test-Path -LiteralPath $QueryPath)) { throw "QueryPath does not exist: $QueryPath" }
if ($CaseAssetManifestPath -and -not (Test-Path -LiteralPath $CaseAssetManifestPath)) { throw "CaseAssetManifestPath does not exist: $CaseAssetManifestPath" }
if ($CaseAssetManifestQueryPath -and -not $CaseAssetManifestPath) { throw "CaseAssetManifestQueryPath requires CaseAssetManifestPath" }
if ($CaseAssetManifestQueryPath -and -not (Test-Path -LiteralPath $CaseAssetManifestQueryPath)) { throw "CaseAssetManifestQueryPath does not exist: $CaseAssetManifestQueryPath" }
if ($CaseAssetReviewDispositionPath -and -not $CaseAssetManifestPath) { throw "CaseAssetReviewDispositionPath requires CaseAssetManifestPath" }
if ($CaseAssetReviewDispositionPath -and -not (Test-Path -LiteralPath $CaseAssetReviewDispositionPath)) { throw "CaseAssetReviewDispositionPath does not exist: $CaseAssetReviewDispositionPath" }

function Invoke-AcquisitionOperation {
    param(
        [Parameter(Mandatory = $true)][ValidateSet("start", "advance", "finish")][string]$Operation,
        [string]$CheckpointPath = ""
    )

    $cargoArgs = @(
        "run", "-p", "bioprism-neurosurgery", "--offline", "--",
        "--research-plan", "--autonomous-acquisition",
        "--autonomous-acquisition-operation", $Operation,
        "--real-glioma", $RealGliomaPath,
        "--public-literature", $PublicLiteraturePath
    )
    if ($QueryPath) { $cargoArgs += @("--autonomous-acquisition-query", $QueryPath) }
    if ($CaseAssetManifestPath) { $cargoArgs += @("--case-asset-manifest", $CaseAssetManifestPath) }
    if ($CaseAssetManifestQueryPath) { $cargoArgs += @("--case-asset-manifest-query", $CaseAssetManifestQueryPath) }
    if ($CaseAssetReviewDispositionPath) { $cargoArgs += @("--autonomous-acquisition-case-asset-review-disposition", $CaseAssetReviewDispositionPath) }
    if ($Operation -eq "advance") {
        $cargoArgs += @("--autonomous-acquisition-session", $CheckpointPath, "--autonomous-acquisition-max-steps", $MaxAdvanceSteps)
    }
    if ($Operation -eq "finish") {
        $cargoArgs += @("--autonomous-acquisition-session", $CheckpointPath)
    }
    $requestJson = Get-Content -Raw -LiteralPath $RequestPath
    $raw = $requestJson | & cargo @cargoArgs 2>$null | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "acquisition operation '$Operation' failed with exit code $LASTEXITCODE"
    }
    try {
        return $raw | ConvertFrom-Json
    } catch {
        throw "acquisition operation '$Operation' returned invalid JSON: $($_.Exception.Message)"
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    function Write-JsonFile {
        param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
        $parent = Split-Path -Parent $Path
        if ($parent -and -not (Test-Path -LiteralPath $parent)) {
            New-Item -ItemType Directory -Path $parent -Force | Out-Null
        }
        $json = $Value | ConvertTo-Json -Depth 100
        [System.IO.File]::WriteAllText(
            [System.IO.Path]::GetFullPath($Path),
            $json,
            [System.Text.UTF8Encoding]::new($false)
        )
    }

    $start = Invoke-AcquisitionOperation -Operation "start"
    Write-JsonFile -Path $StartOutputPath -Value $start
    Write-JsonFile -Path $SessionPath -Value $start.session

    $waves = 0
    $lastAdvance = $null
    while ($true) {
        $checkpoint = Get-Content -Raw -LiteralPath $SessionPath | ConvertFrom-Json
        if ($checkpoint.next_sequence -gt $start.plan.steps.Count) { break }
        if ($waves -ge $MaxWaves) { throw "acquisition worker exceeded MaxWaves before the review hold" }
        $lastAdvance = Invoke-AcquisitionOperation -Operation "advance" -CheckpointPath $SessionPath
        Write-JsonFile -Path $AdvanceOutputPath -Value $lastAdvance
        Write-JsonFile -Path $SessionPath -Value $lastAdvance.session
        $waves++
        if ($lastAdvance.complete) { break }
    }

    $finish = Invoke-AcquisitionOperation -Operation "finish" -CheckpointPath $SessionPath
    if ($CaseAssetManifestPath -and $start.plan.case_asset_report_digest -ne $finish.case_asset_report_digest) {
        throw "acquisition finish lost the case-asset manifest binding"
    }
    if ($CaseAssetReviewDispositionPath -and $start.plan.case_asset_review_disposition_digest -ne $finish.case_asset_review_disposition_digest) {
        throw "acquisition finish lost the case-asset disposition binding"
    }
    Write-JsonFile -Path $FinishOutputPath -Value $finish
    [pscustomobject][ordered]@{
        schema_version = $finish.schema_version
        session_path = $SessionPath
        start_output_path = $StartOutputPath
        advance_output_path = $AdvanceOutputPath
        finish_output_path = $FinishOutputPath
        waves = $waves
        steps_executed = $finish.steps_executed
        human_review_required = $finish.human_review_required
        provider = $finish.provider
        network = $finish.network
    } | ConvertTo-Json -Depth 10
} finally {
    Pop-Location
}
