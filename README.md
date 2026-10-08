# OpenACAI Mod Manager

A Windows mod manager for **Sons of the Forest**, built with Tauri and Svelte. Browse SOTF Mods and Nexus Mods, manage your installed mods, and set up the OpenACAI Endnight Loader.

![OpenACAI Mod Manager](docs/screenshots/openacai-mod-manager-main.png)

## Download

- [Portable app — Windows x64](prebuilt/OpenACAI-Mod-Manager-0.2.0-x64-portable.exe?raw=true)
- [Installer — Windows x64](prebuilt/OpenACAI-Mod-Manager-0.2.0-x64-setup.exe?raw=true)
- [SHA256 checksums](prebuilt/SHA256SUMS.txt)

These downloads are **version 0.2.0**. They don't include the latest source changes; build from source below for those. See the [changelog](CHANGELOG.md) for what's changed.

## Features

- Browse SOTF Mods and Nexus Mods with shared search and filters.
- Install, update, and remove SOTF Mods packages.
- Enable or disable local mods and see which loader they use.
- Read mod descriptions, view screenshots, and check versions and dependencies.
- Spot duplicate installs and compare local versions with the Nexus catalog.
- Install the OpenACAI Endnight Loader from a local ZIP and check its files.

Nexus files are sent to **Vortex** for installation. Packages managed by Vortex must also be enabled, disabled, or removed through Vortex.

## Getting started

1. Open the manager and select your `SonsOfTheForest.exe`.
2. Install the loader from an `OpenACAILoader.zip` package.
3. Open **Mods** to browse the stores or manage installed mods.

Automatic loader downloads are currently unavailable. You'll need a local loader package until the public loader repository is restored.

Nexus browser sign-in requires approval of the manager's application by Nexus Mods. The manual API key option is for development testing.

## Build from source

You'll need Node.js 22.12 or later, Rust, and the [Tauri Windows build prerequisites](https://v2.tauri.app/start/prerequisites/#windows), including the C++ build tools and WebView2.

From the repository folder:

```powershell
npm ci
npm run tauri build
```

The executable is written to `src-tauri/target/release/`; the installer is in `src-tauri/target/release/bundle/nsis/`.

To run the app during development:

```powershell
npm run tauri dev
```

## Credits and license

Maintained by Alex Cooper / OpenACAI. Based on [RedManager](https://github.com/ToniMacaroni/RedManager) by Toni Macaroni.

OpenACAI's changes are licensed under [GPL-3.0-or-later](LICENSE). The original RedManager code is licensed under [Apache-2.0](LICENSES/Apache-2.0.txt). See [third-party notices](THIRD_PARTY_NOTICES.md) for dependency and game-asset credits.
