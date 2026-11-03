# Modstoke rename compatibility

Modstoke Manager is the new display name for OpenACAI Mod Manager. OpenACAI remains the nonprofit publisher, credited as **by OpenACAI**. This rename does not move settings, mods or saves.

## Installed identities

These values stay unchanged so existing installations and integrations remain reachable:

| Identity | Retained value |
| --- | --- |
| Executable | `OpenACAI Mod Manager.exe` |
| npm and Cargo package | `openacai-mod-manager` |
| Bundle ID, WebView data directory and AppUserModelID | `com.alexcooper.openacai.modmanager` |
| Default installation directory | `%LOCALAPPDATA%\OpenACAI Mod Manager` |
| Uninstall registry key | `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenACAI Mod Manager` |
| Installer location and language registry key | `HKCU\Software\alexcooper\OpenACAI Mod Manager` |
| Desktop and Start Menu shortcut filenames | `OpenACAI Mod Manager.lnk` |

The NSIS installer displays **Modstoke Manager**, with **OpenACAI** as publisher. Its custom template keeps the old install directory, registry keys and shortcut filenames. It still reads a previously selected installation directory. Existing shortcuts keep their names; newly created shortcuts have a Modstoke description. The executable target and AppUserModelID stay the same.

WebView's HTTPS origin and the `openacai-mod-hub-*` and `openacai-nexus-*` preferences stay unchanged.

The Nexus integration moved to the new name when it moved to OAuth:
- The Nexus Mods application is **Modstoke**, client ID `modstoke`, replacing the old `openacai-mod-manager` SSO slug.
- Requests send `Application-Name: Modstoke` and a `Modstoke-Manager/<version>` user agent. The version follows the package version, and the user agent names OpenACAI as publisher without claiming that a local build is signed.
- The Nexus refresh token is stored under the `Modstoke Manager` credential service as `nexusmods-refresh-token`. A personal API key left by an earlier build under `OpenACAI Mod Manager` / `nexusmods-api-key` is deleted. No settings migration is required. Ownerless Nexus catalog caches are discarded and rebuilt for the connected account.

The source-built installer is named `Modstoke Manager_<version>_x64-setup.exe`. The 0.3.0 release is published as `Modstoke-Manager-0.3.0-x64-setup.exe` on modstoke.com. The existing signed 0.2.0 files keep their old names and the earlier branding; they have not been rebuilt or re-signed.

## Loader and SDK

The manager displays **Modstoke Loader**, **Modstoke Admin** and **Modstoke SDK**, but retains loader namespaces, assembly names, manifests, configuration keys and command APIs. In particular, it still uses `BepInEx/plugins/OpenACAILoader` and the existing `openacai-loader*.json` files. Its loader package check accepts the core as `Modstoke.Loader.dll`, as current Loader builds ship it, or as `OpenACAI.Endnight.Loader.Core.dll` from older packages. Loader requirement hints accept both the OpenACAI and Modstoke names.

The Loader's own folder in the game root moved from `OpenACAI/` to `Modstoke/` (`Runtime`, `Mods` and `UserData`). The Loader moves existing mods and user data across on its first start. Until then, and for anything it could not move, the Manager follows the Loader's rule: `Modstoke/<folder>` when it exists, otherwise an existing `OpenACAI/<folder>`. Native ZIPs for either layout install into that folder, the console endpoint is read from either `UserData`, and a Loader update removes the old `OpenACAI/Runtime` files listed in its previous file manifest. Mods and user data are never removed.

Upstream projects keep their own names: BepInEx, RedLoader, SonsSdk and `RedLoader.Templates` are not renamed. SOTF Mods and Nexus Mods remain third-party catalogs.

## Coordination with other projects

- Apply the same display names and **by OpenACAI** credit in the loader, admin and SDK repositories. Their releases and assembly identities are outside this manager change. Keep existing runtime paths and package contracts, or provide explicit migrations and aliases before replacing them.
- The Manager is registered with Nexus Mods as Modstoke, client `modstoke`. See the [Nexus integration notes](NEXUS_API_REVIEW.md).
- Set up the modstoke.com website and catalog services separately. The Manager already reads the community server list and the loader release feed from modstoke.com, because the feed's old GitHub address is no longer available. Loader releases are published there from 0.4.1-net11-preview. Mod catalog addresses are unchanged. Before changing any other endpoint, verify manifest schemas, download integrity and support for older clients, and keep old endpoints or redirects available for released clients.
- Sign every new manager build before uploading it, as described in [code signing](SIGNING.md). Signed releases from 0.3.0 are published on modstoke.com; the repository keeps the signed 0.2.0 files in `prebuilt/`. The repository was renamed from `openacai-mod-manager` to `modstoke-manager`; GitHub redirects the old web, clone and download addresses. A download filename change or release relocation also needs stable redirects or aliases for existing links.

## Installer template

`src-tauri/windows/installer.nsi` is based on the [Tauri CLI 2.12.4 template](https://github.com/tauri-apps/tauri/blob/993038c49185307dfc895ab7657d77ca0d5ed7c7/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi), with its MIT license retained alongside it. Local changes separate installed identities from display text. When upgrading the bundler, compare its new template and check install, maintenance and uninstall behavior before replacing this copy.
