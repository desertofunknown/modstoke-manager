# Prebuilt Installers

This folder contains the signed Windows `0.2.0` baseline builds, which retain the OpenACAI Mod Manager name and branding. Build from source for Modstoke Manager, by OpenACAI, and the changes listed under Unreleased in `../CHANGELOG.md`. The binaries and filenames here have not been rebranded.

- `OpenACAI-Mod-Manager-0.2.0-x64-portable.exe`: direct portable app executable.
- `OpenACAI-Mod-Manager-0.2.0-x64-setup.exe`: NSIS setup executable.
- `OpenACAI-Mod-Manager.ico`: OpenACAI Windows icon used by the app and installer.
- `SHA256SUMS.txt`: checksums for the portable executable, setup installer, and icon.

Version `0.2.0` moves loader updates to GitHub Release assets, adds live SHA256 verification progress, caches the latest release manifest for last-known-good comparisons, updates the manager for the .NET 11 loader preview track, and refreshes the UI toward a darker Sons-style options shell.

These are the builds tagged `v0.2.0`. Later rebuilds of 0.2.0 were withdrawn because they bundled Sons of the Forest art, which the Manager no longer ships.

MSI builds are intentionally not published while the current MSI launch issue is investigated.

These builds are signed with the OpenACAI Inc Azure Trusted Signing certificate. Windows SmartScreen and some browsers may still warn until publisher reputation builds, so keep checksums published and verify signatures before release.

Dependency/source links:

- Modstoke Loader package: `https://github.com/desertofunknown/openacai-loader`
- BepInEx: `https://github.com/BepInEx/BepInEx`
- RedLoader/SonsSdk rewrite branch: `https://github.com/ToniMacaroni/RedLoader/tree/rewrite`
- Original RedManager upstream: `https://github.com/ToniMacaroni/RedManager`

Modstoke Admin, the private anti-cheat/admin mod, is not included in this manager or its installers.
