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
- Recheck loader health when the selected game executable changes so the displayed status and repair target follow the active folder.
- Detect whether installed mods are RedLoader packages, RedLoader libraries, BepInEx plugins, native-store installs, manual installs, or Nexus/Vortex-managed packages.
- Browse the live SOTF Mods storefront with category, type, compatibility, search, clearable filters, result counts, a list/grid layout toggle, and installed/online filters that mirror the real store taxonomy.
- Search and filter installed SOTF Mods entries locally without being bounced back to the online catalog.
- Enable or disable installed native/manual mods from the manager with source-aware checkboxes; Vortex-managed packages are detected but left read-only so Vortex deployment metadata is not corrupted.
- Connect a Nexus Mods account through the Nexus browser SSO flow where available, with a manual API token fallback for testing stored locally in the OS credential vault, and browse Nexus/Vortex-side Sons Of The Forest mods with cross-store install status.
- Open Nexus mod detail pages inside the manager to read author directions, choose a specific file from a recommended-first file list, inspect API-listed dependencies for that file, hand the selected file to Vortex through the `nxm://` protocol, and copy the generated Vortex link as a fallback.
- Resolve API-listed Nexus file dependencies against the local Sons Of The Forest inventory, with a capped user-triggered nested dependency check where Nexus returns dependency file IDs, showing installed, missing, and version-review states with direct in-manager details, Nexus, and folder actions where the API/local metadata supports them.
- Preview a selected Nexus file's install plan before Vortex handoff, including inferred or user-selected target placement, action type, file category, dependency review notes, local conflict state, archived-file review state, and Vortex deployment detection.
- Display Nexus premium/supporter/tier status returned by the account validation API, show existing endorsements for downloaded SOTF mods, and let users manually endorse downloaded mods from the online, detail, and installed inventory views.
- Track or untrack Nexus-hosted SOTF mods from the manager, filter the catalog to tracked mods, and review API-listed changelog entries plus semver-aware local update state for installed matches.
- Detect local duplicate/conflict groups across Nexus IDs, manifest names, local package IDs, and Vortex package labels, with a dedicated Conflicts filter, conflict review panel, and detail-drawer conflict panel for matched Nexus entries, including Nexus detail drill-down for conflict rows that carry a Nexus mod ID.
- Triage the Nexus/Vortex catalog with attention, update, disabled, tracked, endorsement-needed, source, mod-type, and conflict filters plus separate online/installed sort modes, clickable health summary cards, an action-queue strip, a visible manual-refresh cooldown, persisted local UI filters, and a clear-filters control.
- Confirm Nexus account writes before tracking/untracking, manual endorsement, or enabling auto-endorse; optional auto-endorse only targets Vortex-managed SOTF downloads with a local user toggle, queue/attempt visibility, once-per-mod attempt tracking, and a small per-refresh cap so the manager does not create excessive Nexus API traffic.
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

The native Mods tab reads from the live SOTF Mods API and currently supports the public storefront categories `Library`, `Misc`, `Model Swap`, and `Quality of Life`, plus type filters for mods, libraries, and builds. Search and compatibility filters update the visible list reactively, while online search still debounces the API reload; the Clear control resets search/category/type/compatibility filters without changing the active online or installed mode, the layout toggle switches between list and grid presentations, and a compact status strip shows visible rows, loaded/installed rows, category count, layout state, and active-filter state.

The Nexus/Vortex tab uses the Nexus Mods API for account validation, Sons Of The Forest game/category metadata, mod feeds, mod details, file choices, file dependency metadata, changelogs, tracked mods, and user endorsement state. Nexus category filters are sourced from the game metadata endpoint when available, with a local Sons Of The Forest taxonomy cache (`Gameplay`, `Miscellaneous`, `Visuals`) and inferred per-mod fallbacks for feed records that omit category data. File choices are sorted with the recommended/main handoff first, while archived/old/removed files are badged for review before Vortex handoff. Selected-file dependencies are compared against the local mod inventory so missing dependencies and version-review cases are visible before handing a file to Vortex; dependency rows that include a Nexus mod ID can open their own in-manager detail drawer with a Back control. A deliberate `Check Nested` action can query up to eight returned dependency file IDs across two nested levels, using cached/user-triggered Nexus dependency calls instead of automatic broad crawling. Nested findings reuse the same local inventory resolver and install-plan notes. The detail drawer also previews an install plan for the chosen file, including inferred or user-selected Sons target placement, action type, file category, file ID/version/uploaded date/size, dependency/conflict review notes, Vortex deployment metadata state, and local conflict remediation when the selected Nexus mod maps to a duplicate local install group. The placement selector can leave the target on Auto or explicitly mark the file as a BepInEx plugin, RedLoader mod, RedLoader library, or manual-review handoff before generating the Vortex plan. The chosen file's generated `nxm://sonsoftheforest/...` link is shown in the drawer and can be copied locally as a Vortex protocol fallback without making extra Nexus calls. The installed inventory derives local duplicate/conflict groups from Nexus IDs, manifest names, package IDs, and Vortex package labels without extra Nexus calls; installed Nexus/Vortex rows can open the same detail drawer from local inventory, and the Local Conflicts shortcut now switches to installed inventory and opens a review panel with grouped installs, source/type/location/version detail, folder actions, optional Nexus detail actions where local metadata carries a Nexus ID, and source-aware enable/disable controls for each conflicting entry. Vortex-managed entries remain read-only in this manager. Attention, update, disabled, tracked, endorsement-needed, source, mod-type, and conflict filters plus online/installed sort modes and a clear-filters control help surface rows that need action before the user scans the full list; the mod-type facet covers BepInEx plugins, RedLoader mods, RedLoader libraries, Nexus/Vortex-managed installs, OpenACAI native installs, and manual/local installs. A compact action queue summarizes endorsement candidates, tracked missing mods, updates, disabled installs, and local conflicts, and each queue chip jumps to the matching filter. Update state comparisons normalize common semver shapes such as `v1.0` versus `1.0.0`, separate remote-newer updates from local-newer or ambiguous version-review cases, and expose the comparison reason in row/detail tooltips. Installed Nexus/Vortex rows reuse user-fetched detail/file metadata for their update verdicts, preferring the recommended file version when the feed-level mod version is coarse. The tab persists the last catalog mode, search, category, install filter, mod-type filter, and sort choices locally so reloads return to the same triage context, and the API note shows when the current Nexus catalog was loaded. Account cards display premium/supporter/tier fields when Nexus exposes them. Tracking, untracking, manual endorsement, and enabling auto-endorse all require an explicit local confirmation before the manager sends a Nexus account write. Endorsement actions are limited to user-installed/downloaded mods, and the optional auto-endorse setting only targets Vortex-managed installs with Nexus mod IDs, shows pending/attempted queue state locally, remembers attempted IDs locally, and caps itself at three attempts per refresh. Public Nexus specs currently do not expose comments or gallery/media endpoints for this flow, so the detail drawer links to the Nexus Description, Files, Posts, Images, and Bugs tabs for those surfaces rather than scraping them. Browser SSO needs a Nexus-approved application slug; until `openacai-mod-manager` is registered by Nexus Mods, the advanced manual token path is only for development/testing builds.

Nexus API usage must follow the [Nexus Mods API Acceptable Use Policy](https://help.nexusmods.com/article/114-api-acceptable-use-policy). This manager sends consistent `Application-Name`, `Application-Version`, and User-Agent metadata, stores user tokens locally instead of on OpenACAI servers, caches Nexus feed/session/detail/file calls for 10 minutes, shows hourly/daily rate-limit meters from the API headers, and rate-limits manual refresh clicks with a visible cooldown to avoid excessive API traffic. It must not bulk scrape, rehost Nexus data, impersonate another application, or ship as a public-facing app that relies on personal API keys instead of a registered Nexus application slug.

The intended Nexus registration path for `openacai-mod-manager` is to use GraphQL for public/read-heavy metadata and supported community surfaces, OAuth2/PKCE for the public desktop login and user-initiated authenticated actions, and legacy REST/v1 only where current mod-manager capabilities are not yet available through GraphQL or OAuth-backed APIs.

To refresh prebuilt files and the ignored local portable test executable:

```powershell
.\scripts\refresh-prebuilt.ps1 -Build
```

## Licensing

OpenACAI-authored manager changes are licensed under `GPL-3.0-or-later`.

The original RedManager project is licensed under `Apache-2.0`; its license text is preserved in `LICENSES/Apache-2.0.txt`, and attribution is preserved in `THIRD_PARTY_NOTICES.md`.

The OpenACAI Endnight Loader packages managed by this app are built on BepInEx and RedLoader/SonsSdk components. Their LGPL notices are preserved by the loader package and called out in this manager's third-party notices. The Main tab links to the public loader repository at `https://github.com/desertofunknown/openacai-loader`.

Selected Sons of the Forest UI textures, splash logo textures, and font assets are bundled into the Tauri frontend so they are not distributed as loose external runtime files. Those assets remain copyright property of Endnight Games and are included only for this open-source, noncommercial Sons of the Forest mod-management integration. They are not available for commercial reuse.
