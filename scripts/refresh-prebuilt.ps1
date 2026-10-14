param(
    [string]$Version = "0.2.0",
    [switch]$Build,
    [switch]$Sign
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$PrebuiltDir = Join-Path $RepoRoot "prebuilt"
$LocalTestDir = Join-Path $RepoRoot "local-test"

if ($Build) {
    Push-Location $RepoRoot
    try {
        npm run tauri build
    }
    finally {
        Pop-Location
    }
}

$PortableSource = Join-Path $RepoRoot "src-tauri\target\release\OpenACAI Mod Manager.exe"
$SetupSource = Join-Path $RepoRoot "src-tauri\target\release\bundle\nsis\Modstoke Manager_${Version}_x64-setup.exe"
$IconSource = Join-Path $RepoRoot "src-tauri\icons\icon.ico"

$Artifacts = @(
    @{
        Source = $PortableSource
        Name = "OpenACAI-Mod-Manager-$Version-x64-portable.exe"
    },
    @{
        Source = $SetupSource
        Name = "OpenACAI-Mod-Manager-$Version-x64-setup.exe"
    },
    @{
        Source = $IconSource
        Name = "OpenACAI-Mod-Manager.ico"
    }
)

New-Item -ItemType Directory -Force -Path $PrebuiltDir | Out-Null
New-Item -ItemType Directory -Force -Path $LocalTestDir | Out-Null
Get-ChildItem -LiteralPath $PrebuiltDir -Filter "OpenACAI-Mod-Manager-*" -File | Remove-Item -Force
Get-ChildItem -LiteralPath $LocalTestDir -Filter "OpenACAI-Mod-Manager-*" -File | Remove-Item -Force

$checksumLines = New-Object System.Collections.Generic.List[string]
$portablePrebuiltPath = $null
foreach ($artifact in $Artifacts) {
    if (-not (Test-Path -LiteralPath $artifact.Source)) {
        throw "Missing build artifact: $($artifact.Source)"
    }

    $prebuiltPath = Join-Path $PrebuiltDir $artifact.Name
    Copy-Item -LiteralPath $artifact.Source -Destination $prebuiltPath -Force

    if ($artifact.Name.EndsWith("-portable.exe", [StringComparison]::OrdinalIgnoreCase)) {
        $portablePrebuiltPath = $prebuiltPath
    }
}

if ($Sign) {
    & (Join-Path $PSScriptRoot "sign-prebuilt.ps1") -Version $Version -NoChecksumUpdate
}

foreach ($artifact in $Artifacts) {
    $prebuiltPath = Join-Path $PrebuiltDir $artifact.Name
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $prebuiltPath).Hash
    $checksumLines.Add("$hash  $($artifact.Name)")
}

$checksumLines | Set-Content -Path (Join-Path $PrebuiltDir "SHA256SUMS.txt") -Encoding ASCII

if ($portablePrebuiltPath) {
    Copy-Item -LiteralPath $portablePrebuiltPath -Destination (Join-Path $LocalTestDir ([System.IO.Path]::GetFileName($portablePrebuiltPath))) -Force
}

Write-Host "Updated prebuilt artifacts and local test executable."
