# OpenACAI Mod Manager

[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Built With: Tauri](https://img.shields.io/badge/built%20with-Tauri-5aa8ff.svg)](https://tauri.app/)
[![Game: Sons Of The Forest](https://img.shields.io/badge/game-Sons%20Of%20The%20Forest-38d68d.svg)](https://store.steampowered.com/app/1326470/Sons_Of_The_Forest/)

Developer: Alex Cooper

OpenACAI Mod Manager is a Tauri 2/Svelte 5 manager for installing and maintaining the OpenACAI Endnight Loader package for Sons of the Forest.

It is based on Toni Macaroni's RedManager, but the main loader flow has been changed for the OpenACAI-maintained Endnight-specific loader:

- Select and install a local `OpenACAILoader.zip`.
- Install or repair the latest public OpenACAI Endnight Loader package from GitHub Release assets using a signed-by-hash update manifest.
- Verify loader-owned files with SHA256 before launching the game, and keep a cached last-known release manifest for offline comparison.
- Detect whether installed mods are RedLoader packages, RedLoader libraries, BepInEx plugins, native-store installs, manual installs, or Nexus/Vortex-managed packages.
- Browse the live SOTF Mods storefront with category, type, compatibility, search, and installed/online filters that mirror the real store taxonomy.
- Search and filter installed SOTF Mods entries locally without being bounced back to the online catalog.
- Enable or disable installed native/manual mods from the manager with source-aware checkboxes; Vortex-managed packages are detected but left read-only so Vortex deployment metadata is not corrupted.
- Connect a Nexus Mods account through the Nexus browser SSO flow where available, with a manual API token fallback for testing stored locally in the OS credential vault, and browse Nexus/Vortex-side Sons Of The Forest mods with cross-store install status.
- Open Nexus mod detail pages inside the manager to read author directions, choose a specific file from a recommended-first file list, inspect API-listed dependencies for that file, and hand the selected file to Vortex through the `nxm://` protocol.
- Resolve API-listed Nexus file dependencies against the local Sons Of The Forest inventory, showing installed, missing, and version-review states with direct Nexus/folder actions.
- Preview a selected Nexus file's install plan before Vortex handoff, including inferred target location, action type, file category, dependency review notes, local conflict state, archived-file review state, and Vortex deployment detection.
- Display Nexus premium/supporter/tier status returned by the account validation API, show existing endorsements for downloaded SOTF mods, and let users manually endorse downloaded mods from the online, detail, and installed inventory views.
- Track or untrack Nexus-hosted SOTF mods from the manager, filter the catalog to tracked mods, and review API-listed changelog entries plus local update state for installed matches.
- Detect local duplicate/conflict groups across Nexus IDs, manifest names, local package IDs, and Vortex package labels, with a dedicated Conflicts filter and conflict review panel in both online and installed views.
- Triage the Nexus/Vortex catalog with attention, update, disabled, tracked, source, and conflict filters plus separate online/installed sort modes and clickable health summary cards.
- Optionally auto-endorse Vortex-managed SOTF downloads with a local user toggle, once-per-mod attempt tracking, and a small per-refresh cap so the manager does not create excessive Nexus API traffic.
- Detect the OpenACAI Endnight Loader core assembly through `BepInEx\plugins\OpenACAILoader`.
- Keep old RedLoader/MelonLoader cleanup affordances so users can avoid competing loader bootstraps.
- Present the manager in a frameless, transparent Sons-style shell with rough/jagged edges, Endnight splash-logo styling, and native-feeling chromatic text effects instead of a standard Windows app frame.

This public manager does not include the private OpenACAI anti-cheat/admin mod.

Brand asset provenance is documented in `BRANDING.md`.
Upstream/fork lineage is documented in `UPSTREAMS.md`.

## Screenshots

![OpenACAI Mod Manager main tab](docs/screenshots/openacai-mod-manager-main.png)

## Prebuilt Downloads

The current Windows builds are published as GitHub Release assets and mirrored in `prebuilt/` for direct repository downloads:

- [Latest GitHub release](https://github.com/desertofunknown/openacai-mod-manager/releases/latest)
- [Windows portable executable](prebuilt/OpenACAI-Mod-Manager-0.2.0-x64-portable.exe)
- [Windows setup executable](prebuilt/OpenACAI-Mod-Manager-0.2.0-x64-setup.exe)
- [OpenACAI icon](prebuilt/OpenACAI-Mod-Manager.ico)
- [SHA256 checksums](prebuilt/SHA256SUMS.txt)

These builds are signed with the OpenACAI Inc Azure Trusted Signing certificate. Windows SmartScreen and some browsers may still warn while publisher reputation builds. Verify the SHA256 checksum and Authenticode signature before release. Code-signing guidance is documented in `SIGNING.md`.

GitHub release assets should mirror this folder when tagged releases are created. MSI builds are intentionally not published while the current MSI launch issue is investigated.

Local test-copy workflow is documented in `LOCAL_TESTING.md`.

## Development

```powershell
npm ci
npm run check
npm run build
npm run tauri build
```

For the desktop shell:

```powershell
npm run tauri dev
```

## Storefront Notes

The native Mods tab reads from the live SOTF Mods API and currently supports the public storefront categories `Library`, `Misc`, `Model Swap`, and `Quality of Life`, plus type filters for mods, libraries, and builds.

The Nexus/Vortex tab uses the Nexus Mods API for account validation, Sons Of The Forest game/category metadata, mod feeds, mod details, file choices, file dependency metadata, changelogs, tracked mods, and user endorsement state. Nexus category filters are sourced from the game metadata endpoint when available, with a local Sons Of The Forest taxonomy cache (`Gameplay`, `Miscellaneous`, `Visuals`) and inferred per-mod fallbacks for feed records that omit category data. File choices are sorted with the recommended/main handoff first, while archived/old/removed files are badged for review before Vortex handoff. Selected-file dependencies are compared against the local mod inventory so missing dependencies and version-review cases are visible before handing a file to Vortex. The detail drawer also previews an install plan for the chosen file, including inferred Sons target location, action type, file category, dependency/conflict review notes, and Vortex deployment metadata state. The installed inventory derives local duplicate/conflict groups from Nexus IDs, manifest names, package IDs, and Vortex package labels without extra Nexus calls; the Conflicts filter opens a review panel with grouped installs, source/type/location/version detail, folder actions, and source-aware enable/disable controls for each conflicting entry. Vortex-managed entries remain read-only in this manager. Attention, update, disabled, tracked, source, and conflict filters plus online/installed sort modes help surface rows that need action before the user scans the full list. Account cards display premium/supporter/tier fields when Nexus exposes them. Endorsement actions are limited to user-installed/downloaded mods, and the optional auto-endorse setting only targets Vortex-managed installs with Nexus mod IDs, remembers attempted IDs locally, and caps itself at three attempts per refresh. Public Nexus specs currently do not expose comments or gallery/media endpoints for this flow, so the manager links to Nexus for those surfaces rather than scraping them. Browser SSO needs a Nexus-approved application slug; until `openacai-mod-manager` is registered by Nexus Mods, the advanced manual token path is only for development/testing builds.

Nexus API usage must follow the [Nexus Mods API Acceptable Use Policy](https://help.nexusmods.com/article/114-api-acceptable-use-policy). This manager sends consistent `Application-Name`, `Application-Version`, and User-Agent metadata, stores user tokens locally instead of on OpenACAI servers, caches Nexus feed/session/detail/file calls for 10 minutes, and rate-limits manual refresh clicks to avoid excessive API traffic. It must not bulk scrape, rehost Nexus data, impersonate another application, or ship as a public-facing app that relies on personal API keys instead of a registered Nexus application slug.

To refresh prebuilt files and the ignored local portable test executable:

```powershell
.\scripts\refresh-prebuilt.ps1 -Build
```

## Licensing

OpenACAI-authored manager changes are licensed under `GPL-3.0-or-later`.

The original RedManager project is licensed under `Apache-2.0`; its license text is preserved in `LICENSES/Apache-2.0.txt`, and attribution is preserved in `THIRD_PARTY_NOTICES.md`.

The OpenACAI Endnight Loader packages managed by this app are built on BepInEx and RedLoader/SonsSdk components. Their LGPL notices are preserved by the loader package and called out in this manager's third-party notices.

Selected Sons of the Forest UI textures, splash logo textures, and font assets are bundled into the Tauri frontend so they are not distributed as loose external runtime files. Those assets remain copyright property of Endnight Games and are included only for this open-source, noncommercial Sons of the Forest mod-management integration. They are not available for commercial reuse.
