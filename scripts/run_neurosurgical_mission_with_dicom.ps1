param(
    [Parameter(Mandatory = $true)]
    [string]$RequestPath,
    [Parameter(Mandatory = $true)]
    [string]$DicomImportPath,
    [Parameter(Mandatory = $true)]
    [string]$RealGliomaPath,
    [string]$MissionQueryPath,
    [string]$FreshnessPath,
    [string]$OutputPath = "work/neurosurgical-mission-dicom.json"
)

$ErrorActionPreference = "Stop"

foreach ($path in @($RequestPath, $DicomImportPath, $RealGliomaPath, $MissionQueryPath, $FreshnessPath)) {
    if (-not [string]::IsNullOrWhiteSpace($path) -and -not [IO.File]::Exists($path)) {
        throw "Input path does not exist: $path"
    }
}

$cargoArgs = @("run", "-p", "bioprism-neurosurgery", "--offline")
$cargoArgs += @("--", "--mission", "--mission-case-dicom", $DicomImportPath, "--real-glioma", $RealGliomaPath)
if (-not [string]::IsNullOrWhiteSpace($MissionQueryPath)) {
    $cargoArgs += @("--mission-query", $MissionQueryPath)
}
if (-not [string]::IsNullOrWhiteSpace($FreshnessPath)) {
    $cargoArgs += @("--mission-freshness", $FreshnessPath)
}

$requestJson = [IO.File]::ReadAllText([IO.Path]::GetFullPath($RequestPath))
$outputText = $requestJson | & cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    throw "DICOM-backed mission CLI failed with exit code $LASTEXITCODE"
}
$mission = $outputText | ConvertFrom-Json
if ($mission.schema -ne "bioprism-neurosurgical-research-mission/0.1" -or
    $mission.provider -ne "none" -or [bool]$mission.network -or
    -not [bool]$mission.human_review_required -or
    $mission.case_dicom_import.schema_version -ne "bioprism-neurosurgery-case-dicom-import/0.1" -or
    $mission.case_asset_manifest.report_digest -ne $mission.case_dicom_import.manifest_report.report_digest -or
    $mission.evidence_synthesis.case_asset_summary.report_digest -ne $mission.case_asset_manifest.report_digest -or
    [int]$mission.mission_audit.fail_count -ne 0) {
    throw "CLI returned an invalid or unbound DICOM-backed mission envelope"
}

$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = [IO.Path]::GetDirectoryName($outputFullPath)
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($outputFullPath, ($outputText -join [Environment]::NewLine), $utf8NoBom)
$validationArgs = @(
    "run", "-p", "bioprism-neurosurgery", "--offline", "--quiet", "--",
    "--validate-mission", $outputFullPath,
    "--real-glioma", $RealGliomaPath,
    "--mission-case-dicom", $DicomImportPath
)
$validationText = $requestJson | & cargo @validationArgs
if ($LASTEXITCODE -ne 0) {
    throw "DICOM-backed mission replay validation failed with exit code $LASTEXITCODE"
}
$validation = ($validationText -join [Environment]::NewLine) | ConvertFrom-Json
if ($validation.valid -ne $true -or $validation.provider -ne "none" -or
    $validation.network -ne $false -or $validation.human_review_required -ne $true) {
    throw "Persisted DICOM-backed mission failed the exact request/snapshot/import replay gate"
}
Write-Output "Wrote and replay-validated $outputFullPath (real DICOM metadata + real glioma mission; human review required)."
