# Manager troubleshooting

This guide covers the [`openacai-tauri2-svelte5-shell` Manager branch](https://github.com/desertofunknown/modstoke-manager/tree/openacai-tauri2-svelte5-shell). The current signed release is [Manager 0.3.0 on modstoke.com](https://modstoke.com/releases/#manager-0-3-0). The older signed 0.2.0 files in `prebuilt/` still carry the OpenACAI Mod Manager name.

## The Manager rejects my game path

Select **Add** and choose the game's `SonsOfTheForest.exe`, not a Steam shortcut or the game folder. The game pages accept only `SonsOfTheForest.exe`, and **Dedicated servers** accepts only `SonsOfTheForestDS.exe`. A folder can hold only one kind of installation, so keep the game and a dedicated server in separate folders. If a saved installation shows as unavailable after the game moved, add its new location and **Forget** the old profile.

## Loader install says the manifest is unavailable

The loader install on the **Loaders** page, **Check files** and **Update / Repair** (labelled **Verify / Repair** until a check finds a problem) all use a [fixed release manifest on modstoke.com](https://modstoke.com/releases/loader/latest/OpenACAILoader.manifest.json). No loader release has been published there yet, so the Manager cannot fetch a new loader package. The 0.2.0 files still request the old GitHub release address, which is no longer available. If a manifest was cached from an earlier successful check, the Manager may show cached status, but it still needs an available package URL to download a repair.

The source contains a local ZIP installer class, but the current UI does not connect it to an install button. Selecting a local `OpenACAILoader.zip` is therefore not a supported workaround in this build. Loader install and repair need a compatible loader release published to that address.

## Launch game stops before the game starts

**Launch game** on Overview starts the game through Steam, so Steam must point to the installation selected in the Manager. If it points elsewhere, the Manager says so and does not start the game. Open that installation's folder and start it the way you usually do. An unfinished package install or pack recovery also blocks the launch until you finish it, for example under **Pack recovery** in **Multiplayer**.

When Modstoke Loader is installed, the Manager checks it before launching. If verification or updating fails, the launch stops and shows an error. While no loader release is published on modstoke.com, this check fails unless an earlier check cached a manifest. You can still start the game through Steam, but that skips the Manager's check and does not repair the loader. Resolve the loader issue before treating its installation as verified.

## Check files reports missing or mismatched loader files

The Manager compares loader-owned files with SHA256 hashes in the release manifest. **Update / Repair** downloads a package and checks its hash before installing it. While the release feed is unavailable, that button cannot fetch the files it needs. Keep the issue details shown by the Manager; they identify the first affected path. Avoid deleting unrelated game or mod files to clear the warning.

## A Nexus mod does not change when I disable or remove it

Nexus file downloads are handed to Vortex. Vortex owns deployments it manages, so enable, disable, update, or remove those packages in Vortex. The Manager can show their state but does not replace Vortex's deployment controls.

Nexus sign-in opens your browser. After you approve Modstoke, Nexus shows "Authorisation successful!". Select **Continue** there and allow the browser to open the Manager. A blank tab may open as well; close it. If the Manager still doesn't respond, select **Having issues?** on the Nexus page, copy the code, and paste it into the field under the sign-in button. Finish within ten minutes of starting, and use the Nexus page from your latest attempt. If the browser did not open at all, use the link the Manager shows under the sign-in button.

## The Sons of the Forest skin shows plain panels

The Manager does not ship the game's art. **Match the game** and **Sons of the Forest** read the frame, title backdrop and font from your own game files, so add your player installation (`SonsOfTheForest.exe`). A dedicated server does not have these files. Until the Manager can read them, it uses the Modstoke skin, and **Settings** shows the reason. The files are cached and read again after the game updates.

## A source build fails

Run `npm ci` from the Manager repository before `npm run check` or `npm run tauri build`. On Windows, the build also needs Rust/Cargo, Visual Studio C++ build tools, the Windows SDK, and WebView2 as described in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#windows). Node.js 22.12 or newer is the version specified by this project. The Manager build does not need a .NET SDK or a copy of Sons of the Forest; those are separate loader-build concerns.

## Check a downloaded executable

Compare its SHA256 hash with the one published with the download. For the 0.3.0 installer it is `511f9a71c16811604a1fb27c49ac8f613395f0138ff78685b668c334b7b691b0`; the 0.2.0 files are listed in the [checksums committed alongside them](https://github.com/desertofunknown/modstoke-manager/blob/openacai-tauri2-svelte5-shell/prebuilt/SHA256SUMS.txt). In PowerShell, replace the path with the file you downloaded:

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath ".\Modstoke-Manager-0.3.0-x64-setup.exe"
Get-AuthenticodeSignature -LiteralPath ".\Modstoke-Manager-0.3.0-x64-setup.exe"
```

The signature should report `Valid` with **OpenACAI Inc** as the signer. From 0.3.0, the installed `OpenACAI Mod Manager.exe` and its uninstaller are signed too. The program that the 0.2.0 setup installs reports `NotSigned`. A local source build is not signed unless it is signed separately.

