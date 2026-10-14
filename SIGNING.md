# Code Signing

The public Windows builds of Modstoke Manager, by OpenACAI, should be signed before release. OpenACAI remains the legal publisher and signing identity. Checksums help users verify that a download matches this repository, but checksums do not replace Authenticode signing.

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
Thumbprint: 8402188618D8D3CC02A6F885EE2AB8B7A5E5481A
Serial: 330001AA67AE1A924D42711E1300000001AA67
Identity validation: d00cf1a3-c3a3-4890-8b2e-c1576d258506
```

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

Then sign the current prebuilts:

```powershell
.\scripts\sign-prebuilt.ps1
```

For a locally installed certificate/private key or hardware-token certificate, use the thumbprint path:

```powershell
$env:CODESIGN_CERT_THUMBPRINT = "8402188618D8D3CC02A6F885EE2AB8B7A5E5481A"
.\scripts\sign-prebuilt.ps1 -SigningMode Thumbprint
```

If the certificate is installed in the machine store instead of the current-user store:

```powershell
$env:CODESIGN_CERT_STORE_LOCATION = "LocalMachine"
```

Or rebuild, refresh, sign, and rewrite checksums in one command:

```powershell
.\scripts\refresh-prebuilt.ps1 -Build -Sign
```

The signing script:

- Finds the latest installed x64 Windows SDK `signtool.exe`.
- Downloads `nuget.exe` locally if needed.
- Restores `Microsoft.ArtifactSigning.Client` into `.tools/artifact-signing`.
- Generates an ignored metadata JSON file for the account/profile when Azure signing is used.
- Can sign through Azure Artifact Signing or a locally installed certificate selected by thumbprint.
- Signs the portable and setup EXEs with SHA256 and Microsoft timestamping.
- Verifies signatures and refreshes `prebuilt/SHA256SUMS.txt`.

Do not commit private keys, certificate passwords, service-principal secrets, generated metadata files, or token credentials.

## Current Mitigation

- Publish SHA256 checksums in `prebuilt/SHA256SUMS.txt`.
- Provide a clearly branded portable executable and NSIS setup installer.
- Do not publish the MSI while its launch issue is unresolved.
