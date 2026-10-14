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
| Nexus credential service | `OpenACAI Mod Manager` |
| Nexus SSO slug | `openacai-mod-manager` |

The NSIS installer displays **Modstoke Manager**, with **OpenACAI** as publisher. Its custom template keeps the old install directory, registry keys and shortcut filenames. It still reads a previously selected installation directory. Existing shortcuts keep their names; newly created shortcuts have a Modstoke description. The executable target and AppUserModelID stay the same.

WebView's HTTPS origin, `openacai-mod-hub-*` and `openacai-nexus-*` preferences, credential account, Nexus request headers and user agent also stay unchanged. No settings migration is required.

The source-built installer is named `Modstoke Manager_<version>_x64-setup.exe`. Published download aliases and the existing signed 0.2.0 files keep their old names. Those signed files still contain the earlier branding; they have not been rebuilt or re-signed as part of this rename.

## Loader and SDK

The manager displays **Modstoke Loader**, **Modstoke Admin** and **Modstoke SDK**, but retains loader namespaces, assembly names, manifests, configuration keys and command APIs. In particular, it still uses `BepInEx/plugins/OpenACAILoader`, `OpenACAI.Endnight.Loader.Core.dll`, and the existing `openacai-loader*.json` files. Loader requirement hints accept both the OpenACAI and Modstoke names.

Upstream projects keep their own names: BepInEx, RedLoader, SonsSdk and `RedLoader.Templates` are not renamed. SOTF Mods and Nexus Mods remain third-party catalogs.

## Coordination with other projects

- Apply the same display names and **by OpenACAI** credit in the loader, admin and SDK repositories. Their releases and assembly identities are outside this manager change. Keep existing runtime paths and package contracts, or provide explicit migrations and aliases before replacing them.
- Coordinate the manager's display name with Nexus Mods while retaining the registered integration identifiers until replacement identifiers are approved.
- Set up **modstoke.com**, HTTPS and the replacement website/catalog services separately. Verify manifest schemas, download integrity and support for older clients before changing any endpoint. Keep old endpoints or redirects available for released clients.
- Publish newly signed manager builds separately. A GitHub repository rename, download filename change or release relocation also needs stable redirects or aliases for existing links.

## Installer template

`src-tauri/windows/installer.nsi` is based on the [Tauri CLI 2.11.2 template](https://github.com/tauri-apps/tauri/blob/499df79be65ef8c0670abc0207cd9e37b55d8491/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi), with its MIT license retained alongside it. Local changes separate installed identities from display text. When upgrading the bundler, compare its new template and check install, maintenance and uninstall behavior before replacing this copy.
