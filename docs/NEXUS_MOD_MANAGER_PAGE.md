# OpenACAI Mod Manager Nexus Page Draft

## Mod Name

OpenACAI Mod Manager

## Suggested Category

Primary new category request: **Mod Managers & Tools**

Fallback if Nexus only allows existing Sons Of The Forest categories: **Miscellaneous**

Suggested companion category for the loader page: **Mod Loaders & Frameworks**

## Short Description

A Sons Of The Forest mod manager for installing, repairing, verifying, and browsing OpenACAI Endnight Loader, RedLoader-style mods, BepInEx plugins, native SOTF mods, and Vortex/Nexus-managed installs.

## Long Description

OpenACAI Mod Manager is an open-source Sons Of The Forest mod manager built by OpenACAI Inc / Alex Cooper. It is designed to pair with OpenACAI Endnight Loader and make loader installation, repair, verification, and mod browsing easier for players who do not want to manually manage every file in their game directory.

The manager is based on Toni Macaroni's RedManager lineage, but has been rebuilt around the OpenACAI Endnight Loader workflow, a modern Tauri 2 / Svelte 5 frontend, signed release builds, live loader file verification, and mod-type awareness for the different Sons Of The Forest mod ecosystems.

The goal is simple: give Sons Of The Forest players one focused tool that understands this one game well.

## Current Features

- Installs, repairs, and uninstalls OpenACAI Endnight Loader from public GitHub release assets.
- Verifies loader-owned files with SHA256 checks before launch.
- Detects installed loader version and compares it against the latest known release manifest.
- Detects RedLoader-style mods, RedLoader libraries, BepInEx plugins, native SOTF store installs, manual installs, and Vortex/Nexus-managed packages.
- Browses the SOTF Mods storefront with search, categories, type filters, compatibility labels, and installed/online views.
- Enables or disables native/manual installed mods from the manager while preserving Vortex-managed deployments as read-only.
- Connects to Nexus Mods/Vortex flows where available, with local credential storage and cache/rate-limit behavior designed to respect the Nexus API acceptable use policy.
- Shows Nexus account/profile status when the API exposes it.
- Opens Nexus mod detail views inside the manager for author notes, file choices, dependency metadata, changelogs, tracking, and endorsement actions where supported by the public API.
- Keeps Nexus catalog previews visible in compact layouts and cycles through multiple API-provided preview images in catalog rows and detail drawers when they are available.
- Separates Nexus files, dependency checks, changelogs, and install-plan panels into responsive scroll regions so smaller windows remain usable.
- Hands selected Nexus files to Vortex through `nxm://` links when Vortex is installed and registered.
- Uses a Sons-style dark menu shell with rough edges, chromatic text highlights, and bundled UI assets instead of a plain Windows utility window.

## What This Is Not

This is not a private anti-cheat/admin tool package. It does not include the private OpenACAI admin panel or private anti-cheat mod.

This is also not an official Endnight Games product. Some Sons Of The Forest visual assets are used for this open-source, noncommercial mod-management integration with permission, but those assets remain copyright property of Endnight Games and are not available for commercial reuse.

## Requirements

- Windows 10 or Windows 11.
- A legal Steam install of Sons Of The Forest.
- Internet access for release manifest checks, loader downloads, SOTF Mods catalog browsing, and Nexus/Vortex features.
- Microsoft Edge WebView2 Runtime if it is not already present on the system.
- Vortex is optional, but required for Vortex/Nexus deployment handoff.

## Source And Compile Instructions

All source links, upstream credits, required build tools, and reviewer compile
steps are documented here:

https://github.com/desertofunknown/openacai-mod-manager/blob/openacai-tauri2-svelte5-shell/docs/NEXUS_SOURCE_AND_BUILD.md

## Installation

1. Download the latest signed OpenACAI Mod Manager release.
2. Run the portable executable or installer.
3. Select your `SonsOfTheForest.exe` if the manager does not detect it automatically.
4. Use the main tab to install or repair OpenACAI Endnight Loader.
5. Use **Check** or **Verify / Repair** to confirm installed loader files match the expected release hashes.
6. Launch the game normally or from the manager.

## Updating

Use the manager's **Verify / Repair** flow whenever a loader update is available or when loader-owned files do not match the release manifest. The manager compares installed files against the latest release metadata and only offers a loader update when local files are missing, mismatched, or older than the known public release.

## Nexus And Vortex Notes

OpenACAI Mod Manager is being built as a focused one-game Nexus/Vortex companion for Sons Of The Forest. It is intended to support mod browsing, install-state detection, file selection, dependency awareness, tracking, endorsement, and Vortex handoff without excessive API calls.

Dependency checks resolve selected game-scoped Nexus file IDs to v3 mod-file IDs before requesting materialized dependency metadata, with a fallback to the original selected file ID if the v3 resolver endpoint is unavailable.

Nexus features must follow the Nexus Mods API acceptable use policy. The manager identifies itself with application headers, caches feed/detail/session data, rate-limits manual refresh actions, and stores user credentials locally through the OS credential vault. Public browser SSO requires a Nexus-approved application slug before it should be treated as production-ready.

## Known Limitations

- Nexus/Vortex integration is still being expanded. Vortex remains the source of truth for Vortex-managed deployments.
- The manager can detect and hand files to Vortex, but full Vortex-equivalent dependency resolution and conflict deployment are still in active development.
- Nexus public API surfaces do not currently expose every community page feature, such as full comments/gallery browsing, so the manager links out to Nexus where the API does not provide supported endpoints.
- Some mod-type detection relies on manifests and folder structure. Mods with incomplete metadata may need manual review.

## Credits

- Developer: Alex Cooper / OpenACAI Inc.
- Original RedManager lineage: Toni Macaroni.
- RedLoader and SonsSdk compatibility reference: Toni Macaroni and contributors.
- BepInEx runtime foundation: BepInEx contributors.
- Sons Of The Forest UI/art assets: Endnight Games, used only for this open-source, noncommercial Sons Of The Forest mod-management project.

## License

OpenACAI-authored manager changes are licensed under **GPL-3.0-or-later**.

Original RedManager code is licensed under **Apache-2.0**. Third-party and upstream notices are preserved in the repository.

Sons Of The Forest visual assets remain copyright property of Endnight Games and are not licensed for commercial reuse.

## Suggested Tags

mod manager, loader installer, OpenACAI, Endnight Loader, Sons Of The Forest, RedLoader, BepInEx, Vortex, Nexus Mods, SOTF Mods
