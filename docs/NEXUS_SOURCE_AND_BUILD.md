# Nexus source and build notes

This document covers Modstoke Manager source. The Manager preview is under
review and has no current public download. The Loader, SDK, and client
companion are built and reviewed as a separate package.

## Source and credits

- Manager source: https://github.com/desertofunknown/openacai-mod-manager
- Upstream RedManager by Toni Macaroni: https://github.com/ToniMacaroni/RedManager

The source commit named with a future Manager download must match that
download's build inputs. The public review branch may lag this local preview
until its source changes are accepted and published.

OpenACAI-authored Manager changes are licensed under GPL-3.0-or-later. The
original RedManager source retains its Apache-2.0 license. See `LICENSE`,
`NOTICE`, `THIRD_PARTY_NOTICES.md`, `FRONTEND_THIRD_PARTY_NOTICES.txt`,
`RUST_THIRD_PARTY_NOTICES.txt`, and `LICENSES/` for component terms.

The Manager uses original CSS and system fonts. This source checkout does not
bundle Sons of the Forest UI textures, splash logos, or fonts.

## Build the Manager on Windows

Install Node.js 22.12 or newer, Rust/Cargo, and the
[Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows),
including Visual Studio C++ build tools and WebView2. The Manager build does
not require the game or a .NET SDK.

From the root of a checked-out Manager source tree:

```powershell
npm ci
npm run check
$env:VITE_MODSTOKE_RELEASE_INDEX_URL = 'https://modstoke.com/release-index.json'
$env:VITE_MODSTOKE_PREVIEW_BUILD = '1'
npm run build
$env:CARGO_ENCODED_RUSTFLAGS = "--remap-path-prefix=$env:USERPROFILE=/modstoke-build"
npm run tauri build -- --config src-tauri/tauri.preview.conf.json
```

The preview Tauri configuration skips its own frontend build step, so run
`npm run build` first. The unsigned NSIS setup is written under
`src-tauri/target/release/bundle/nsis/`. See
[the preview build guide](PREVIEW_BUILD.md) for the same source build flow.
The Cargo flag remaps user-profile source paths that Rust may otherwise embed
in the executable. A binary still needs a local-path scan after packaging.

To record hashes for the resulting local setup files:

```powershell
Get-ChildItem -Path 'src-tauri/target/release/bundle/nsis/*.exe' | Get-FileHash -Algorithm SHA256
```

## Loader package

The Manager reads the Modstoke website release index. Guided installation and
repair remain unavailable until it selects a complete, reviewed Loader, SDK,
and client companion bundle. This Manager has no local Loader ZIP picker.
Loader package build instructions and any generated game interop assemblies
belong with the separate Loader source and release review; they are not
Manager build inputs.
