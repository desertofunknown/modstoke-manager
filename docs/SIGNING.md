# Code Signing

Every Windows binary of Modstoke Manager, by OpenACAI, must be signed before it is committed to this repository or attached to a GitHub release. OpenACAI remains the legal publisher and signing identity. Checksums help users verify that a download matches this repository, but they do not replace Authenticode signing.

## Azure Artifact Signing

Microsoft has rebranded Azure Trusted Signing as Azure Artifact Signing. Use a Public Trust certificate profile for public GitHub downloads and SmartScreen reputation.

Azure setup, done in the Azure portal:

1. Create an Artifact Signing account in a supported region.
2. Assign yourself `Artifact Signing Identity Verifier` so you can submit identity validation.
3. Create an Organization/Public identity validation for the legal OpenACAI nonprofit entity.
4. Complete the identity and document checks from the email/Azure portal flow.
5. Create a Public Trust certificate profile from the completed identity validation.
6. Assign the release signer identity `Artifact Signing Certificate Profile Signer` on the account or profile.

Identity validation requires legal nonprofit/business details and can take days. Keep the Azure nonprofit grant subscription selected if the grant is intended to cover signing costs.

## Local Signing

The OpenACAI public signing identity is:

```text
CN=OpenACAI Inc, O=OpenACAI Inc, L=Farmington, S=Missouri, C=US
Identity validation: d00cf1a3-c3a3-4890-8b2e-c1576d258506
```

Artifact Signing uses short-lived certificates, so the thumbprint and serial change between signings. Check the subject, not the thumbprint. The v0.2.0 files were signed with a certificate valid from 4 to 7 June 2026, thumbprint `8402188618D8D3CC02A6F885EE2AB8B7A5E5481A`.

The identity validation ID is an Azure-side validation record. It is not the same thing as the certificate profile name used by SignTool.

For Azure Artifact Signing, set these environment variables before signing:

```powershell
$env:AZURE_ARTIFACT_SIGNING_ENDPOINT = "https://eus.codesigning.azure.net"
$env:AZURE_ARTIFACT_SIGNING_ACCOUNT_NAME = "your-artifact-signing-account"
$env:AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE = "your-public-profile"
```

Optional:

```powershell
$env:AZURE_ARTIFACT_SIGNING_CORRELATION_ID = "openacai-mod-manager-0.2.0"
$env:SIGNTOOL_EXE = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe"
$env:AZURE_ARTIFACT_SIGNING_DLIB = "C:\path\to\Azure.CodeSigning.Dlib.dll"
```

Then sign the new files in `prebuilt/`. Do not re-sign the v0.2.0 files already there, because that changes their checksums so they no longer match the v0.2.0 tag:

```powershell
.\scripts\sign-prebuilt.ps1
```

For a certificate installed on this PC with its private key, or on a hardware token, use its thumbprint. Azure Artifact Signing certificates cannot be used this way.

```powershell
$env:CODESIGN_CERT_THUMBPRINT = "<thumbprint of the installed certificate>"
.\scripts\sign-prebuilt.ps1 -SigningMode Thumbprint
```

If the certificate is installed in the machine store instead of the current-user store:

```powershell
$env:CODESIGN_CERT_STORE_LOCATION = "LocalMachine"
```

Or build, copy, sign and rewrite checksums in one command. The script defaults to version 0.2.0, so first raise the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, then pass the same version:

```powershell
.\scripts\refresh-prebuilt.ps1 -Version 0.3.0 -Build -Sign
```

The script stops if the build fails or an output is missing, before it touches `prebuilt/`. If signing stops partway, restore the old files with `git restore prebuilt`.

The signing script:

- Uses `SIGNTOOL_EXE`, or the latest installed x64 Windows SDK `signtool.exe`.
- For Azure signing, downloads `nuget.exe` if needed, restores `Microsoft.ArtifactSigning.Client` into `.tools/artifact-signing` and writes an ignored metadata file there.
- Can sign through Azure Artifact Signing or a locally installed certificate selected by thumbprint.
- Signs the portable and setup EXEs in `prebuilt/` with SHA256 and Microsoft timestamping.
- Verifies the signatures and refreshes `prebuilt/SHA256SUMS.txt`.

The script signs only those two files. The setup is built before they are signed, so the `OpenACAI Mod Manager.exe` and `uninstall.exe` it installs are unsigned. This is true of the signed v0.2.0 setup as well. To sign them too, give the release build a Tauri sign command: set `bundle.windows.signCommand` to a SignTool command ending in `%1`. The bundler then signs the app before packing it, and also signs the uninstaller and the setup.

Do not commit private keys, certificate passwords, service-principal secrets, generated metadata files, or token credentials.

## Before uploading binaries

Every Windows binary committed to `prebuilt/` or attached to a GitHub release must be signed by OpenACAI Inc.

- Check each file with `Get-AuthenticodeSignature` or `signtool verify /pa`. The status must be `Valid`, with **OpenACAI Inc** as the signer.
- Do not commit `prebuilt/` after running `refresh-prebuilt.ps1` without `-Sign`. Restore the signed files with `git restore prebuilt`.
- Publish SHA256 checksums in `prebuilt/SHA256SUMS.txt`, generated after signing.
- Do not publish an MSI while its launch issue is unresolved. The build currently makes only the NSIS setup.

`prebuilt/` holds the signed v0.2.0 builds, which still carry the OpenACAI Mod Manager name and branding. New signed releases are on hold, so keep those files unchanged until a new build can be signed.
