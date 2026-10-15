# Modstoke Manager Nexus Page Draft

## Mod Name

Modstoke Manager

## Suggested Category

Primary new category request: **Mod Managers & Tools**

Fallback if Nexus only allows existing Sons Of The Forest categories: **Miscellaneous**

Suggested companion category for the loader page: **Mod Loaders & Frameworks**

## Short Description

A Sons Of The Forest mod manager, by OpenACAI, for browsing mods and checking installed Loader files. Guided Loader installation and repair await a complete published bundle.

## Long Description

Modstoke Manager is an open-source Sons Of The Forest mod manager by OpenACAI. It is designed to pair with Modstoke Loader and make loader verification and mod browsing easier for players. Guided Loader installation and repair will become available when a complete bundle is published through the website release index.

The manager is based on Toni Macaroni's RedManager lineage, but has been rebuilt around the Modstoke Loader workflow, a modern Tauri 2 / Svelte 5 frontend, live loader file verification, and mod-type awareness for the different Sons Of The Forest mod ecosystems.

Modstoke is the website and catalog brand, with `modstoke.com` as its intended domain. Registration and deployment are separate work. Existing repository and service links remain in use.

The earlier signed `0.2.0` build retains the OpenACAI Mod Manager name and branding. The newer Modstoke-branded preview is an unsigned local build pending release review.

The goal is simple: give Sons Of The Forest players one focused tool that understands this one game well.

## Current Features

- Checks the Modstoke website release index for a complete Loader, SDK, and client companion bundle. Guided installation remains unavailable until a reviewed bundle is selected.
- Verifies loader-owned files with SHA256 checks when a game path is selected or when the user runs **Check**.
- Detects installed loader version and compares it against the latest known release manifest.
- Detects RedLoader-style mods, RedLoader libraries, BepInEx plugins, native SOTF store installs, manual installs, and Vortex/Nexus-managed packages.
- Browses the SOTF Mods storefront from a unified Mods page with search, categories, type filters, compatibility labels, and installed/online views.
- Enables or disables native/manual installed mods from the manager while preserving Vortex-managed deployments as read-only.
- Switches the same Mods page to Nexus Mods/Vortex flows where available, with local credential storage and cache/rate-limit behavior designed to respect the Nexus API acceptable use policy.
- Shows Nexus account/profile status when the API exposes it.
- Opens Nexus mod detail views inside the manager for author notes, file choices, dependency metadata, changelogs, tracking, and endorsement actions where supported by the public API.
- Keeps Nexus catalog previews visible in compact layouts and cycles through multiple API-provided preview images in catalog rows and detail drawers when they are available.
- Separates Nexus files, dependency checks, changelogs, and install-plan panels into responsive scroll regions so smaller windows remain usable.
- Hands selected Nexus files to Vortex through `nxm://` links when Vortex is installed and registered.
- Uses a dark menu shell with rough edges and chromatic text highlights.

## What This Is Not

This is not a private anti-cheat/admin tool package. It does not include Modstoke Admin, the private admin panel and anti-cheat mod.

This is not an official Endnight Games product. The manager uses original CSS and system fonts rather than game textures, logos, or fonts.

## Requirements

- Windows 10 or Windows 11.
- A legal Steam install of Sons Of The Forest.
- Internet access for SOTF Mods catalog browsing and Nexus/Vortex features.
- Microsoft Edge WebView2 Runtime if it is not already present on the system.
- Vortex is optional, but required for Vortex/Nexus deployment handoff.

## Source And Compile Instructions

Manager source, upstream credit, build tools, and reviewer compile steps are
documented in this source checkout. The exact preview source commit must be
published before this draft is used for a download page:

https://github.com/desertofunknown/openacai-mod-manager

## Installation

1. Build the current preview from its reviewed source when available. Earlier signed `0.2.0` builds are different historical builds with an unavailable Loader feed.
2. Run the portable executable or installer.
3. Select your `SonsOfTheForest.exe` if the manager does not detect it automatically.
4. If the Manager shows a published complete Loader bundle, use its guided install action. The current preview leaves this action unavailable while the release index is empty.
5. Use **Check** to inspect installed loader files. **Verify / Repair** requires a published bundle and matching release hashes.
6. Launch the game normally or from the manager.

## Updating

The Manager compares installed files against release metadata. **Verify / Repair** requires a complete Loader bundle selected in the website release index; it is unavailable while the index has no selected bundle.

## Nexus And Vortex Notes

Modstoke Manager is being built as a focused one-game Nexus/Vortex companion for Sons Of The Forest. The top-level Mods page now switches between the SOTF Mods storefront and the Nexus/Vortex source so players can browse either storefront from one workspace. It is intended to support mod browsing, install-state detection, file selection, dependency awareness, tracking, endorsement, and Vortex handoff without excessive API calls.

Dependency checks resolve game-scoped file IDs through the v3 `mod-file-versions` endpoint, then request materialized dependency ranges using the returned global version ID. Global version IDs are kept separate from parent file-group IDs and v1 catalog UIDs. Requirements retain their compatible file alternatives; one matching enabled install can satisfy a requirement. Unresolved choices and version ranges remain visible for review. Recursive checks continue in bounded batches and keep unfinished lookups available for retry.

Nexus features must follow the Nexus Mods API acceptable use policy. The manager identifies itself with application headers, caches feed/detail/session data, rate-limits manual refresh actions, and stores user credentials locally through the OS credential vault. Public browser SSO requires a Nexus-approved application slug before it should be treated as production-ready.

## Known Limitations

- Guided Loader installation and repair are unavailable until the website selects a complete Loader, SDK, and client companion bundle. The Manager has no local ZIP picker.
- Nexus/Vortex integration is still being expanded. Vortex remains the source of truth for Vortex-managed deployments.
- The manager can detect and hand files to Vortex, but full Vortex-equivalent dependency resolution and conflict deployment are still in active development.
- Nexus public API surfaces do not currently expose every community page feature, such as full comments/gallery browsing, so the manager links out to Nexus where the API does not provide supported endpoints.
- Some mod-type detection relies on manifests and folder structure. Mods with incomplete metadata may need manual review.

## Credits

- Developer: Alex Cooper / OpenACAI Inc.
- Original RedManager lineage: Toni Macaroni.
- RedLoader and SonsSdk compatibility reference: Toni Macaroni and contributors.
- BepInEx runtime foundation: BepInEx contributors.

## License

OpenACAI-authored manager changes are licensed under **GPL-3.0-or-later**.

Original RedManager code is licensed under **Apache-2.0**. Third-party and upstream notices are preserved in the repository.


## Suggested Tags

mod manager, loader installer, Modstoke, Modstoke Loader, OpenACAI, Sons Of The Forest, RedLoader, BepInEx, Vortex, Nexus Mods, SOTF Mods
