# Manager preview build

This source checkout builds the unsigned `0.2.1-preview.4` Windows installer for release
review. It is not a published release. The interface uses original CSS and
system fonts; the old game-derived textures, splash logos, font, and signed
`0.2.0` prebuilt binaries are absent from this branch. Historical commits and
the default branch are unchanged.

On Windows, install the Node.js, Rust, C++ and WebView2 prerequisites listed in
the [README](../README.md). Then build the frontend and Tauri installer from a
clean checkout:

```powershell
npm ci
$env:VITE_MODSTOKE_RELEASE_INDEX_URL = 'https://modstoke.com/release-index.json'
$env:VITE_MODSTOKE_PREVIEW_BUILD = '1'
npm run build
$env:CARGO_ENCODED_RUSTFLAGS = "--remap-path-prefix=$env:USERPROFILE=/modstoke-build"
npm run tauri build -- --config src-tauri/tauri.preview.conf.json
```

The Tauri preview configuration intentionally skips its `beforeBuildCommand`,
so the frontend build above is required. The website URL must be the exact
allowed `https://modstoke.com/release-index.json` address. With an empty
release index, the Manager shows that no complete Loader, SDK, and client
companion bundle is published, and guided installation stays unavailable.
The Rust path-remap flag keeps the builder's user-profile path out of compiler
source locations embedded in the Windows executable. Scan the completed
installer and executable for local paths before distributing any build.

The setup is written to `src-tauri/target/release/bundle/nsis/`. It includes
`LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`,
`FRONTEND_THIRD_PARTY_NOTICES.txt`, `RUST_THIRD_PARTY_NOTICES.txt`, and the
other license texts listed in the preview configuration. The installer is
unsigned. Building it does not publish or sign any artifact.
