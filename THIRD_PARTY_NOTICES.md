# Third-Party Notices

Modstoke Manager, formerly OpenACAI Mod Manager, is developed by Alex Cooper and published by OpenACAI.

This project is based on and/or manages packages that include:

- **RedManager** by Toni Macaroni, licensed under Apache-2.0. Source: `https://github.com/ToniMacaroni/RedManager`
- **BepInEx** by the BepInEx team, licensed under LGPL-2.1. Source: `https://github.com/BepInEx/BepInEx`
- **RedLoader/SonsSdk** by Toni Macaroni, licensed under LGPL-2.1 according to the upstream repository license file. Source: `https://github.com/ToniMacaroni/RedLoader`
- **Tauri NSIS installer template**, by the Tauri contributors and Cargo-Bundle contributors, licensed under MIT. The complete notices and license are preserved in `src-tauri/windows/License_MIT.md`.

## Frontend JavaScript and icons

The Manager frontend includes Svelte, Tauri JavaScript API and plugins, lodash, semver, and Svelte runtime dependencies. Its generated SVG icons come from Lucide and SVG Spinners through Iconify and unplugin-icons. Some of the Lucide icons are derived from Feather and carry an additional MIT notice. Package versions, copyright notices, license texts, and icon sources are recorded in `FRONTEND_THIRD_PARTY_NOTICES.txt`.

## Windows Rust dependencies

The Windows executable includes Tauri and other Rust libraries. Package names,
versions, license declarations, source links, and available license and notice
texts for the pinned Windows runtime dependency tree are recorded in
`RUST_THIRD_PARTY_NOTICES.txt`.

OpenACAI-authored manager changes are licensed as `GPL-3.0-or-later`.

The Apache-2.0 text is preserved in `LICENSES/Apache-2.0.txt`. The LGPL-2.1 text is preserved in `LICENSES/LGPL-2.1.txt`.
