# Manager troubleshooting

This guide covers the [`openacai-tauri2-svelte5-shell` Manager branch](https://github.com/desertofunknown/openacai-mod-manager/tree/openacai-tauri2-svelte5-shell). The signed 0.2.0 downloads in that branch still carry the OpenACAI Mod Manager name.

## The Manager rejects my game path

Choose the game's `SonsOfTheForest.exe`, not the Steam shortcut or the install folder itself. The file picker accepts an `.exe`, and the Manager checks that its filename is `SonsOfTheForest.exe`.

## Loader install says the manifest is unavailable

The current install, Check, and Update / Repair flows use a [hard-coded GitHub release manifest](https://github.com/desertofunknown/openacai-loader/releases/latest/download/OpenACAILoader.manifest.json). That old loader repository is unavailable at present, so the Manager cannot fetch a new loader package from it. If a manifest was cached from an earlier successful check, the Manager may show cached status, but it still needs an available package URL to download a repair.

The source contains a local ZIP installer class, but the current UI does not connect it to an install button. Selecting a local `OpenACAILoader.zip` is therefore not a supported workaround in this build. Loader install and repair require a compatible public release feed or a Manager code change.

## Start Game stops after a loader update error

When a loader is installed, the Manager checks it before its **Start Game** action. If verification or updating fails, that action stops and shows an error. You can launch the game through Steam, but that bypasses the Manager's prelaunch check and does not repair the loader. Resolve the loader issue before treating its installation as verified.

## Check reports missing or mismatched loader files

The Manager compares loader-owned files with SHA256 hashes in the release manifest. **Update / Repair** downloads a package and checks its hash before installing it. While the release feed is unavailable, that button cannot fetch the files it needs. Keep the issue details shown by the Manager; they identify the first affected path. Avoid deleting unrelated game or mod files to clear the warning.

## A Nexus mod does not change when I disable or remove it

Nexus file downloads are handed to Vortex. Vortex owns deployments it manages, so enable, disable, update, or remove those packages in Vortex. The Manager can show their state but does not replace Vortex's deployment controls.

Nexus browser sign-in needs approval of the Manager application by Nexus Mods. The manual API key field is for development testing; do not include your key in a bug report.

## A source build fails

Run `npm ci` from the Manager repository before `npm run check` or `npm run tauri build`. On Windows, the build also needs Rust/Cargo, Visual Studio C++ build tools, the Windows SDK, and WebView2 as described in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#windows). Node.js 22.12 or newer is the version specified by this project. The Manager build does not need a .NET SDK or a copy of Sons of the Forest; those are separate loader-build concerns.

## Check a downloaded executable

Compare its SHA256 hash with the [checksums committed alongside the downloads](https://github.com/desertofunknown/openacai-mod-manager/blob/openacai-tauri2-svelte5-shell/prebuilt/SHA256SUMS.txt). In PowerShell, replace the path with the file you downloaded:

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath ".\OpenACAI-Mod-Manager-0.2.0-x64-setup.exe"
Get-AuthenticodeSignature -LiteralPath ".\OpenACAI-Mod-Manager-0.2.0-x64-setup.exe"
```

The signature should report `Valid` with **OpenACAI Inc** as the signer. A local source build will not carry that signature unless it is signed separately.

