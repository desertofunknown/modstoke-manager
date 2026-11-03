# Prebuilt Installers

This folder contains the signed Windows `0.2.0` builds, which keep the OpenACAI Mod Manager name in the app and the filenames. A newer signed release is not available yet. For Modstoke Manager and the changes listed under Unreleased in the [changelog](../CHANGELOG.md), build from source; a source build is unsigned.

- `OpenACAI-Mod-Manager-0.2.0-x64-portable.exe`: direct portable app executable.
- `OpenACAI-Mod-Manager-0.2.0-x64-setup.exe`: NSIS setup executable.
- `OpenACAI-Mod-Manager.ico`: the OpenACAI icon used by the 0.2.0 app and installer. Current source builds use the Modstoke icon.
- `SHA256SUMS.txt`: checksums for the portable executable, setup installer, and icon.

Version `0.2.0` moves loader updates to GitHub Release assets, adds live SHA256 verification progress, caches the latest release manifest for last-known-good comparisons, updates the manager for the .NET 11 loader preview track, and refreshes the UI toward a darker Sons-style options shell. The GitHub release address these builds use for the loader is no longer available, so they cannot install or repair the loader.

These are the builds tagged `v0.2.0`. Later rebuilds of 0.2.0 were withdrawn because they bundled Sons of the Forest art, which the Manager no longer ships.

MSI builds are intentionally not published while the current MSI launch issue is investigated.

These builds are signed by OpenACAI Inc through Azure Trusted Signing (now Azure Artifact Signing). Windows SmartScreen and some browsers may still warn until the publisher builds reputation. To check a download, see [Check a downloaded executable](../docs/TROUBLESHOOTING.md#check-a-downloaded-executable).

Every executable in this folder must be signed before it is committed or pushed. Build and sign with `scripts/refresh-prebuilt.ps1 -Build -Sign`; without `-Sign` the script copies unsigned builds here. Before pushing, check that `Get-AuthenticodeSignature` reports `Valid` with OpenACAI Inc as the signer for each `.exe`, and that `SHA256SUMS.txt` matches the files. See [SIGNING.md](../SIGNING.md).

Dependency/source links:

- Modstoke Loader package: published through `https://modstoke.com/releases/loader/latest/OpenACAILoader.manifest.json` (no release has been published there yet)
- BepInEx: `https://github.com/BepInEx/BepInEx`
- RedLoader/SonsSdk rewrite branch: `https://github.com/ToniMacaroni/RedLoader/tree/rewrite`
- Original RedManager upstream: `https://github.com/ToniMacaroni/RedManager`

Modstoke Admin, the private anti-cheat/admin mod, is not included in this manager or its installers.
