# Modstoke Manager

A Windows toolkit for Sons of the Forest, **by OpenACAI**. Manage local mods and loaders, browse mod catalogs, and look after dedicated servers in one application.

Modstoke Manager is based on [RedManager](https://github.com/ToniMacaroni/RedManager) by Toni Macaroni. The current source uses Tauri 2 and Svelte 5. The signed 0.2.0 downloads still contain the earlier OpenACAI branding; build the current branch for the newer workspaces and Nexus integration changes.

## Nexus Mods and Vortex

The Nexus workspace provides catalog search and filters, mod details, file selection, dependency information and changelogs. Tracking and endorsements are explicit actions. Selected downloads open in Vortex through `nxm://` links; Vortex remains responsible for its own installation and deployment.

Catalog requests use the connected account. A refresh replaces the previous catalog instead of retaining rows omitted by Nexus. Account changes discard cached requests, and the local catalog snapshot is restricted to its account and expires after 24 hours. Cached results are labelled and can predate changes to account preferences.

Public account sign-in is awaiting Nexus application registration. Personal API keys are available only for development testing and review, stored locally in the operating system's credential vault. See the [Nexus integration notes](NEXUS_API_REVIEW.md) for the registration proposal and review steps.

## Requirements

- Windows 10 or 11 and Microsoft Edge WebView2 Runtime.
- A Steam installation of Sons of the Forest for game and mod management.
- Internet access for online catalogs.
- Vortex installed and registered for Nexus download handoff.

## Source and builds

[Current Manager source](https://github.com/desertofunknown/openacai-mod-manager/tree/openacai-tauri2-svelte5-shell) · [Build instructions](NEXUS_SOURCE_AND_BUILD.md)

Run the Manager, select `SonsOfTheForest.exe`, and open **Installed mods**, **Browse mods**, **Loaders** or **Dedicated servers** for the corresponding tools. The [README](../README.md) describes the available downloads and setup.

## Current limitations

- Public Nexus OAuth sign-in is not implemented or approved yet.
- Loader install and repair are unavailable until the existing release feed is restored or replaced with a compatible service. The current UI does not provide a local ZIP installation flow.
- Vortex conflict rules and deployment remain in Vortex. Dependency information does not replace its deployment checks.
- Features not exposed by the supported Nexus APIs link to the Nexus website.
- Modstoke's website and catalog, with `modstoke.com` as the intended address, are separate work. Existing working service addresses remain in use.

## Credits and licenses

Developed by Alex Cooper / OpenACAI Inc. Original RedManager lineage by Toni Macaroni. RedLoader and SonsSdk compatibility references are by Toni Macaroni and contributors; BepInEx is by its contributors.

OpenACAI-authored Manager changes are licensed under GPL-3.0-or-later. Original RedManager code retains its Apache-2.0 license. See [third-party notices](../THIRD_PARTY_NOTICES.md) for the retained licenses and credits.

Bundled Sons of the Forest UI assets remain copyright property of Endnight Games and have project-specific, noncommercial permission for this tool. They are not covered by the Manager's GPL license. Modstoke Manager is independent and is not endorsed by Endnight Games or Nexus Mods. Modstoke Admin is a separate project and is not included in Manager downloads.
