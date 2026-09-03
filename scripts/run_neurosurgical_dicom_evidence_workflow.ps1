param(
    [Parameter(Mandatory = $true)]
    [string]$RequestPath,
    [Parameter(Mandatory = $true)]
    [string]$DicomImportPath,
    [string]$RealGliomaPath,
    [string]$PublicLiteraturePath,
    [string]$QueryPath,
    [string]$OutputPath = "work/dicom-evidence-workflow.json"
)

$ErrorActionPreference = "Stop"

foreach ($path in @($RequestPath, $DicomImportPath, $RealGliomaPath, $PublicLiteraturePath, $QueryPath)) {
    if (-not [string]::IsNullOrWhiteSpace($path) -and -not [IO.File]::Exists($path)) {
        throw "Input path does not exist: $path"
    }
}
if ([string]::IsNullOrWhiteSpace($RealGliomaPath) -and [string]::IsNullOrWhiteSpace($PublicLiteraturePath)) {
    throw "Supply -RealGliomaPath for glioma or -PublicLiteraturePath for a non-glioma lane"
}
$cargoArgs = @("run", "-p", "bioprism-neurosurgery", "--offline")
$cargoArgs += @("--", "--case-dicom-evidence-workflow", $DicomImportPath)
if (-not [string]::IsNullOrWhiteSpace($RealGliomaPath)) {
    $cargoArgs += @("--real-glioma", $RealGliomaPath)
}
if (-not [string]::IsNullOrWhiteSpace($PublicLiteraturePath)) {
    $cargoArgs += @("--public-literature", $PublicLiteraturePath)
}
if (-not [string]::IsNullOrWhiteSpace($QueryPath)) {
    $cargoArgs += @("--case-dicom-evidence-workflow-query", $QueryPath)
}

$requestJson = [IO.File]::ReadAllText([IO.Path]::GetFullPath($RequestPath))
$outputText = $requestJson | & cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    throw "DICOM evidence workflow CLI failed with exit code $LASTEXITCODE"
}
$report = $outputText | ConvertFrom-Json
if ($report.schema_version -ne "bioprism-neurosurgery-case-dicom-evidence-workflow/0.1" -or
    $report.provider -ne "none" -or [bool]$report.network -or
    -not [bool]$report.human_review_required -or [bool]$report.synthetic_data) {
    throw "CLI returned an invalid provider-free, human-review workflow envelope"
}

$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = [IO.Path]::GetDirectoryName($outputFullPath)
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($outputFullPath, ($outputText -join [Environment]::NewLine), $utf8NoBom)
Write-Output "Wrote $outputFullPath (DICOM metadata + source-bound evidence workers; human review required)."
