param(
    [string]$Version = "",
    [string[]]$Files = @(),
    [string]$SigningMode = $env:OPENACAI_SIGNING_MODE,
    [string]$Endpoint = $env:AZURE_ARTIFACT_SIGNING_ENDPOINT,
    [string]$AccountName = $env:AZURE_ARTIFACT_SIGNING_ACCOUNT_NAME,
    [string]$CertificateProfileName = $env:AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE,
    [string]$CorrelationId = $env:AZURE_ARTIFACT_SIGNING_CORRELATION_ID,
    [string]$CertificateThumbprint = $env:CODESIGN_CERT_THUMBPRINT,
    [string]$CertificateStoreLocation = $env:CODESIGN_CERT_STORE_LOCATION,
    [string]$CertificateStoreName = $env:CODESIGN_CERT_STORE_NAME,
    [string]$SignToolPath = $env:SIGNTOOL_EXE,
    [string]$DlibPath = $env:AZURE_ARTIFACT_SIGNING_DLIB,
    [string]$TimestampUrl = "http://timestamp.acs.microsoft.com",
    [string]$Description = "Modstoke Manager by OpenACAI",
    [string]$ToolsDir = "",
    [switch]$NoChecksumUpdate
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$PrebuiltDir = Join-Path $RepoRoot "prebuilt"

if ([string]::IsNullOrWhiteSpace($ToolsDir)) {
    $ToolsDir = Join-Path $RepoRoot ".tools\artifact-signing"
}

if ([string]::IsNullOrWhiteSpace($SigningMode)) {
    $SigningMode = "Auto"
}

if ([string]::IsNullOrWhiteSpace($CertificateStoreLocation)) {
    $CertificateStoreLocation = "CurrentUser"
}

if ([string]::IsNullOrWhiteSpace($CertificateStoreName)) {
    $CertificateStoreName = "My"
}

function Get-ProjectVersion {
    $packageJson = Get-Content -LiteralPath (Join-Path $RepoRoot "package.json") -Raw | ConvertFrom-Json
    return $packageJson.version
}

function Require-Value {
    param(
        [string]$Name,
        [string]$Value
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "Missing $Name. Set the matching AZURE_ARTIFACT_SIGNING_* environment variable or pass the parameter explicitly."
    }
}

function Normalize-Thumbprint {
    param([string]$Thumbprint)
    return ($Thumbprint -replace "[^0-9A-Fa-f]", "").ToUpperInvariant()
}

function Has-AzureSigningMetadata {
    return -not [string]::IsNullOrWhiteSpace($Endpoint) `
        -and -not [string]::IsNullOrWhiteSpace($AccountName) `
        -and -not [string]::IsNullOrWhiteSpace($CertificateProfileName)
}

function Resolve-SigningMode {
    switch ($SigningMode.ToLowerInvariant()) {
        "azure" {
            return "Azure"
        }
        "thumbprint" {
            return "Thumbprint"
        }
        "local" {
            return "Thumbprint"
        }
        "auto" {
            if (Has-AzureSigningMetadata) {
                return "Azure"
            }

            if (-not [string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
                return "Thumbprint"
            }

            throw "No signing configuration found. Set Azure Artifact Signing variables or set CODESIGN_CERT_THUMBPRINT for a locally installed certificate."
        }
        default {
            throw "Unsupported signing mode '$SigningMode'. Use Auto, Azure, or Thumbprint."
        }
    }
}

function Get-LatestSignTool {
    if (-not [string]::IsNullOrWhiteSpace($SignToolPath)) {
        if (-not (Test-Path -LiteralPath $SignToolPath)) {
            throw "SIGNTOOL_EXE does not exist: $SignToolPath"
        }

        return (Resolve-Path -LiteralPath $SignToolPath).Path
    }

    $windowsKitBin = "C:\Program Files (x86)\Windows Kits\10\bin"
    if (-not (Test-Path -LiteralPath $windowsKitBin)) {
        throw "Windows SDK SignTool was not found. Install Windows SDK Build Tools or set SIGNTOOL_EXE."
    }

    $candidate = Get-ChildItem -LiteralPath $windowsKitBin -Recurse -Filter "signtool.exe" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "\\x64\\signtool\.exe$" } |
        Sort-Object @{ Expression = {
            try {
                [version]$_.Directory.Parent.Name
            } catch {
                [version]"0.0.0.0"
            }
        }; Descending = $true } |
        Select-Object -First 1

    if (-not $candidate) {
        throw "x64 SignTool was not found. Install Windows SDK Build Tools or set SIGNTOOL_EXE."
    }

    return $candidate.FullName
}

function Ensure-NuGet {
    New-Item -ItemType Directory -Force -Path $ToolsDir | Out-Null
    $nugetPath = Join-Path $ToolsDir "nuget.exe"

    if (-not (Test-Path -LiteralPath $nugetPath)) {
        Write-Host "Downloading NuGet CLI for local Artifact Signing client restore..."
        Invoke-WebRequest -Uri "https://dist.nuget.org/win-x86-commandline/latest/nuget.exe" -OutFile $nugetPath
    }

    return $nugetPath
}

function Get-ArtifactSigningDlib {
    if (-not [string]::IsNullOrWhiteSpace($DlibPath)) {
        if (-not (Test-Path -LiteralPath $DlibPath)) {
            throw "AZURE_ARTIFACT_SIGNING_DLIB does not exist: $DlibPath"
        }

        return (Resolve-Path -LiteralPath $DlibPath).Path
    }

    $existing = Get-ChildItem -LiteralPath $ToolsDir -Recurse -Filter "Azure.CodeSigning.Dlib.dll" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "\\bin\\x64\\Azure\.CodeSigning\.Dlib\.dll$" } |
        Sort-Object FullName -Descending |
        Select-Object -First 1

    if ($existing) {
        return $existing.FullName
    }

    $nugetPath = Ensure-NuGet
    Write-Host "Restoring Microsoft.ArtifactSigning.Client locally..."
    & $nugetPath install Microsoft.ArtifactSigning.Client -x -OutputDirectory $ToolsDir -NonInteractive | Write-Host
    if ($LASTEXITCODE -ne 0) {
        throw "NuGet failed to restore Microsoft.ArtifactSigning.Client."
    }

    $restored = Get-ChildItem -LiteralPath $ToolsDir -Recurse -Filter "Azure.CodeSigning.Dlib.dll" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "\\bin\\x64\\Azure\.CodeSigning\.Dlib\.dll$" } |
        Sort-Object FullName -Descending |
        Select-Object -First 1

    if (-not $restored) {
        throw "Azure.CodeSigning.Dlib.dll was not found after restoring Microsoft.ArtifactSigning.Client."
    }

    return $restored.FullName
}

function Resolve-SigningFiles {
    if (-not $Files -or $Files.Count -eq 0) {
        $Files = @(
            (Join-Path $PrebuiltDir "OpenACAI-Mod-Manager-$Version-x64-portable.exe"),
            (Join-Path $PrebuiltDir "OpenACAI-Mod-Manager-$Version-x64-setup.exe")
        )
    }

    $resolved = New-Object System.Collections.Generic.List[string]
    foreach ($file in $Files) {
        $pathToResolve = $file
        if (-not [System.IO.Path]::IsPathRooted($pathToResolve)) {
            $pathToResolve = Join-Path $RepoRoot $pathToResolve
        }

        if (-not (Test-Path -LiteralPath $pathToResolve)) {
            throw "File to sign was not found: $pathToResolve"
        }

        $resolved.Add((Resolve-Path -LiteralPath $pathToResolve).Path)
    }

    return $resolved
}

function New-MetadataFile {
    param(
        [string]$MetadataPath
    )

    $metadata = [ordered]@{
        Endpoint = $Endpoint.Trim()
        CodeSigningAccountName = $AccountName.Trim()
        CertificateProfileName = $CertificateProfileName.Trim()
    }

    if (-not [string]::IsNullOrWhiteSpace($CorrelationId)) {
        $metadata.CorrelationId = $CorrelationId.Trim()
    }

    if (-not [string]::IsNullOrWhiteSpace($env:AZURE_ARTIFACT_SIGNING_EXCLUDE_CREDENTIALS)) {
        $metadata.ExcludeCredentials = @(
            $env:AZURE_ARTIFACT_SIGNING_EXCLUDE_CREDENTIALS.Split(",") |
                ForEach-Object { $_.Trim() } |
                Where-Object { $_ }
        )
    }

    $metadata | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $MetadataPath -Encoding UTF8
}

function Find-LocalCertificate {
    Require-Value "CODESIGN_CERT_THUMBPRINT" $CertificateThumbprint
    $thumbprint = Normalize-Thumbprint $CertificateThumbprint
    $locations = if ($CertificateStoreLocation -eq "Auto") {
        @("CurrentUser", "LocalMachine")
    } else {
        @($CertificateStoreLocation)
    }
    $searchedStores = New-Object System.Collections.Generic.List[string]

    foreach ($location in $locations) {
        $storePath = "Cert:\$location\$CertificateStoreName"
        $searchedStores.Add("$location\$CertificateStoreName")
        if (-not (Test-Path -LiteralPath $storePath)) {
            continue
        }

        $certificate = Get-ChildItem -LiteralPath $storePath |
            Where-Object { (Normalize-Thumbprint $_.Thumbprint) -eq $thumbprint } |
            Select-Object -First 1

        if ($certificate) {
            return [pscustomobject]@{
                Certificate = $certificate
                Location = $location
                StoreName = $CertificateStoreName
            }
        }
    }

    throw "Certificate thumbprint $thumbprint was not found in $($searchedStores -join ', '). Import the PFX/certificate with private key, attach the signing token, or use Azure Artifact Signing endpoint/account/profile values."
}

function Update-Checksums {
    $checksumFiles = @(
        (Join-Path $PrebuiltDir "OpenACAI-Mod-Manager-$Version-x64-portable.exe"),
        (Join-Path $PrebuiltDir "OpenACAI-Mod-Manager-$Version-x64-setup.exe"),
        (Join-Path $PrebuiltDir "OpenACAI-Mod-Manager.ico")
    )

    $checksumLines = New-Object System.Collections.Generic.List[string]
    foreach ($checksumFile in $checksumFiles) {
        if (-not (Test-Path -LiteralPath $checksumFile)) {
            continue
        }

        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $checksumFile).Hash
        $checksumLines.Add("$hash  $([System.IO.Path]::GetFileName($checksumFile))")
    }

    $checksumLines | Set-Content -Path (Join-Path $PrebuiltDir "SHA256SUMS.txt") -Encoding ASCII
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-ProjectVersion
}

$resolvedSigningMode = Resolve-SigningMode
$signTool = Get-LatestSignTool
$filesToSign = Resolve-SigningFiles
$dlib = $null
$metadataPath = $null
$localCertificate = $null

if ($resolvedSigningMode -eq "Azure") {
    Require-Value "AZURE_ARTIFACT_SIGNING_ENDPOINT" $Endpoint
    Require-Value "AZURE_ARTIFACT_SIGNING_ACCOUNT_NAME" $AccountName
    Require-Value "AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE" $CertificateProfileName

    $net8Runtime = dotnet --list-runtimes | Select-String -Pattern "^Microsoft\.NETCore\.App 8\."
    if (-not $net8Runtime) {
        Write-Warning "Artifact Signing Client Tools require the .NET 8 runtime. Install it if signing fails during dlib load."
    }

    $dlib = Get-ArtifactSigningDlib
    New-Item -ItemType Directory -Force -Path $ToolsDir | Out-Null
    $metadataPath = Join-Path $ToolsDir "metadata.generated.json"
    New-MetadataFile -MetadataPath $metadataPath
} else {
    $localCertificate = Find-LocalCertificate
    if (-not $localCertificate.Certificate.HasPrivateKey) {
        throw "Certificate $($localCertificate.Certificate.Thumbprint) was found in $($localCertificate.Location)\$($localCertificate.StoreName), but it does not expose a private key for signing."
    }
}

foreach ($file in $filesToSign) {
    Write-Host "Signing $file"
    if ($resolvedSigningMode -eq "Azure") {
        & $signTool sign /v /debug /fd SHA256 /tr $TimestampUrl /td SHA256 /d $Description /dlib $dlib /dmdf $metadataPath $file
    } else {
        $thumbprint = Normalize-Thumbprint $CertificateThumbprint
        $signArgs = @("sign", "/v", "/debug", "/fd", "SHA256", "/tr", $TimestampUrl, "/td", "SHA256", "/d", $Description, "/s", $localCertificate.StoreName, "/sha1", $thumbprint)
        if ($localCertificate.Location -eq "LocalMachine") {
            $signArgs += "/sm"
        }

        $signArgs += $file
        & $signTool @signArgs
    }

    if ($LASTEXITCODE -ne 0) {
        throw "SignTool failed while signing $file"
    }

    & $signTool verify /pa /v $file
    if ($LASTEXITCODE -ne 0) {
        throw "SignTool verification failed for $file"
    }
}

if (-not $NoChecksumUpdate) {
    Update-Checksums
}

Write-Host "Signed Modstoke Manager prebuilts."
