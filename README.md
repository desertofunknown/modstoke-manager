# Modstoke Manager

Modstoke Manager is a Windows mod manager for **Sons of the Forest**, maintained by OpenACAI. It is based on [RedManager](https://github.com/ToniMacaroni/RedManager) by Toni Macaroni. The app brings the SOTF Mods and Nexus Mods catalogs together, shows installed mods, and manages Modstoke Loader files.

This local source checkout contains the unsigned, website-connected
`0.2.1-preview.4` candidate. Its changes are not yet on the public review
branch. The repository name and some installed paths still use OpenACAI names
for compatibility. See the [rename compatibility notes](docs/BRAND_COMPATIBILITY.md)
and [changelog](CHANGELOG.md).

## Download the Windows app

This preview has no current public Manager download. The
Modstoke-branded preview is unsigned and remains in release review. Earlier
signed `0.2.0` binaries are historical builds and are not the source of this
preview. See [how to verify a downloaded executable](docs/TROUBLESHOOTING.md#check-a-downloaded-executable).

## Get started

1. Run an approved Manager build on Windows 10 or 11. WebView2 is required; the installer is configured to bootstrap it if needed.
2. Select `SonsOfTheForest.exe` in your game installation folder.
3. Open **Mods** to browse SOTF Mods and Nexus Mods or inspect your installed mods. Nexus file installation is handed to Vortex. Manage Vortex deployments in Vortex.

**Guided Loader installation is currently unavailable.** This preview checks
the Modstoke website release index for a complete Loader, SDK, and client
companion bundle. Until one is selected, the install and repair actions stay
disabled. It does not provide a local ZIP picker. Older Manager builds use an
unavailable GitHub release feed. See [troubleshooting](docs/TROUBLESHOOTING.md).

Nexus browser sign-in also depends on Nexus Mods approving the Manager application. The manual API key option is intended for development testing.

## Develop on Windows

Install Node.js 22.12 or newer, Rust/Cargo, and the [Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows), including Visual Studio C++ build tools and WebView2. You can build the Manager without installing the game or a .NET SDK. Building the separate loader package has different requirements. From the root of this source checkout:

```powershell
npm ci
npm run check
npm run tauri dev
```

For a local Windows build:

```powershell
npm run tauri build
```

The executable is written under `src-tauri/target/release/`; the NSIS installer is under `src-tauri/target/release/bundle/nsis/`. Local builds are unsigned. `npm run build` builds only the web frontend. For this preview's exact website URL and warning banner, follow the [preview build instructions](docs/PREVIEW_BUILD.md).

The Svelte frontend lives in `src/`, and the Rust/Tauri backend in `src-tauri/`. Loader, SDK, and Admin are separate projects. Admin is not included in this repository or its Manager downloads.

## Credits and licenses

OpenACAI-authored Manager changes are licensed under [GPL-3.0-or-later](LICENSE). The RedManager source retains its [Apache-2.0 license](LICENSES/Apache-2.0.txt). See the [third-party notices](THIRD_PARTY_NOTICES.md) and [upstream lineage](UPSTREAMS.md) for other components. The preview uses original CSS and system fonts instead of game textures, splash logos, or fonts. Modstoke Manager is independent and is not endorsed by Endnight Games.

