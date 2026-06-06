# Prebuilt Installers

This folder contains the current Windows desktop builds of OpenACAI Mod Manager.

- `OpenACAI-Mod-Manager-0.2.0-x64-portable.exe`: direct portable app executable.
- `OpenACAI-Mod-Manager-0.2.0-x64-setup.exe`: NSIS setup executable.
- `OpenACAI-Mod-Manager.ico`: OpenACAI Windows icon used by the app and installer.
- `SHA256SUMS.txt`: checksums for the portable executable, setup installer, and icon.

Version `0.2.0` moves loader updates to GitHub Release assets, adds live SHA256 verification progress, caches the latest release manifest for last-known-good comparisons, updates the manager for the .NET 11 loader preview track, refreshes the UI toward a darker Sons-style options shell, links the Main tab to the public loader repository, and expands the Nexus/Vortex and SOTF Mods flows. The current signed prebuilts include path-aware loader health checks, a validated/icon path selector, Nexus category taxonomy caching, profile/endorsement/tracking/changelog controls, a visible Nexus manual-refresh cooldown, auto-endorse queue state, dependency readiness, local conflict detection with grouped review panels, source-aware state actions, detail-drawer conflict panels for matched Nexus mods, installed-row Nexus detail drawers, Nexus detail links for Description/Files/Posts/Images/Bugs, reactive catalog search/category/install filters with a clear-filters control, catalog attention filters/sorts with active health-card shortcuts, recommended-first file choices with archived-file review badges, install-plan previews with reactive action labels before Vortex handoff, installed SOTF Mods local search/facet persistence with clearable filters, result counts, a list/grid layout toggle, and stricter Vortex-managed update guards.

MSI builds are intentionally not published while the current MSI launch issue is investigated.

These builds are signed with the OpenACAI Inc Azure Trusted Signing certificate. Windows SmartScreen and some browsers may still warn until publisher reputation builds, so keep checksums published and verify signatures before release.

Dependency/source links:

- OpenACAI Endnight Loader package: `https://github.com/desertofunknown/openacai-loader`
- BepInEx: `https://github.com/BepInEx/BepInEx`
- RedLoader/SonsSdk rewrite branch: `https://github.com/ToniMacaroni/RedLoader/tree/rewrite`
- Original RedManager upstream: `https://github.com/ToniMacaroni/RedManager`

The private OpenACAI anti-cheat/admin mod is not included in this manager or its installers.
