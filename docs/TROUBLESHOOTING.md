# Manager troubleshooting

This guide covers the unsigned `0.2.1-preview.4` source candidate. Its exact
source commit has not been published to the public review branch.
There is no public download of this preview yet. Older signed `0.2.0` builds
still carry the OpenACAI Mod Manager name and use a different Loader feed.

## The Manager rejects my game path

Choose the game's `SonsOfTheForest.exe`, not the Steam shortcut or the install folder itself. The file picker accepts an `.exe`, and the Manager checks that its filename is `SonsOfTheForest.exe`.

## Guided Loader installation is unavailable

This preview checks `https://modstoke.com/release-index.json` for a selected
complete Loader, SDK, and client companion bundle. If none is selected, it
shows that guided installation is unavailable and disables repair. A website
error also stops new installation; cached status cannot authorize a new
download. Use **Check website again** after the feed is restored.

Older signed `0.2.0` builds use an [old GitHub release manifest](https://github.com/desertofunknown/openacai-loader/releases/latest/download/OpenACAILoader.manifest.json)
that is unavailable. Neither build offers a working local ZIP picker. A local
`OpenACAILoader.zip` is not a supported workaround in the current UI.

## Start Game stops after a loader update error

Older `0.2.0` builds check an installed loader before their **Start Game**
action and stop if the old release feed cannot verify it. The website-connected
preview does not auto-update the Loader before starting the game. It keeps
guided install and repair unavailable until a complete bundle is published.
Launching through Steam does not repair Loader files.

## Check reports missing or mismatched loader files

The Manager compares loader-owned files with SHA256 hashes in the selected
release manifest. **Update / Repair** needs a published package whose ZIP and
files match those hashes. Keep the issue details shown by the Manager; they
identify the first affected path. Avoid deleting unrelated game or mod files
to clear the warning.

## A Nexus mod does not change when I disable or remove it

Nexus file downloads are handed to Vortex. Vortex owns deployments it manages, so enable, disable, update, or remove those packages in Vortex. The Manager can show their state but does not replace Vortex's deployment controls.

Nexus browser sign-in needs approval of the Manager application by Nexus Mods. The manual API key field is for development testing; do not include your key in a bug report.

## A source build fails

Run `npm ci` from the Manager repository before `npm run check` or `npm run tauri build`. On Windows, the build also needs Rust/Cargo, Visual Studio C++ build tools, the Windows SDK, and WebView2 as described in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#windows). Node.js 22.12 or newer is the version specified by this project. The Manager build does not need a .NET SDK or a copy of Sons of the Forest; those are separate loader-build concerns.

## Check a downloaded executable

Check the hash against the checksum published for that exact download. In
PowerShell, replace the path with the file you downloaded:

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath ".\Manager-setup.exe"
Get-AuthenticodeSignature -LiteralPath ".\Manager-setup.exe"
```

The `0.2.1-preview.4` build is unsigned, so it will not report a valid
publisher signature. Do not confuse it with the earlier signed `0.2.0` build.

