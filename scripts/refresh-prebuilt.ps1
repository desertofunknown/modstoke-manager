param(
    [string]$Version = "",
    [switch]$Build,
    [switch]$Sign
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$PrebuiltDir = Join-Path $RepoRoot "prebuilt"
$LocalTestDir = Join-Path $RepoRoot "local-test"

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = (Get-Content -LiteralPath (Join-Path $RepoRoot "package.json") -Raw | ConvertFrom-Json).version
}

$BuildStarted = Get-Date
if ($Build) {
    # Local folder names (the checkout, the cargo cache, the user profile) stay out of the binary.
    # The encoded form allows the spaces in this repository's path. Later prefixes win.
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
    $remaps = @(
        "--remap-path-prefix=$env:USERPROFILE=~",
        "--remap-path-prefix=$cargoHome=/cargo",
        "--remap-path-prefix=$RepoRoot=/src"
    )
    $previousFlags = $env:CARGO_ENCODED_RUSTFLAGS
    $env:CARGO_ENCODED_RUSTFLAGS = $remaps -join [char]0x1f
    Push-Location $RepoRoot
    try {
        npm run tauri build
        if ($LASTEXITCODE -ne 0) {
            throw "npm run tauri build failed (exit $LASTEXITCODE)"
        }
    }
    finally {
        Pop-Location
        $env:CARGO_ENCODED_RUSTFLAGS = $previousFlags
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

foreach ($artifact in $Artifacts) {
    if (-not (Test-Path -LiteralPath $artifact.Source)) {
        throw "Missing build artifact: $($artifact.Source)"
    }
    if ($Build -and $artifact.Source -ne $IconSource -and (Get-Item -LiteralPath $artifact.Source).LastWriteTime -lt $BuildStarted) {
        throw "Stale build artifact: $($artifact.Source)"
    }
}

New-Item -ItemType Directory -Force -Path $PrebuiltDir | Out-Null
New-Item -ItemType Directory -Force -Path $LocalTestDir | Out-Null
Get-ChildItem -LiteralPath $PrebuiltDir -Filter "OpenACAI-Mod-Manager-*" -File | Remove-Item -Force
Get-ChildItem -LiteralPath $LocalTestDir -Filter "OpenACAI-Mod-Manager-*" -File | Remove-Item -Force

$checksumLines = New-Object System.Collections.Generic.List[string]
$portablePrebuiltPath = $null
foreach ($artifact in $Artifacts) {
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
