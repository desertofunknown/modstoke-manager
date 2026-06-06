# Nexus Source And Build Instructions

This document is for Nexus Mods review of the OpenACAI Mod Manager and the
OpenACAI Endnight Loader package it installs for Sons Of The Forest.

The project is open source. The manager installs and verifies a loader package,
so the source links and build notes below include the manager, the loader, and
the loader-side forks used to produce a usable package.

## Public Source Links

OpenACAI source repositories:

- OpenACAI Mod Manager:
  https://github.com/desertofunknown/openacai-mod-manager/tree/openacai-tauri2-svelte5-shell
- OpenACAI Endnight Loader:
  https://github.com/desertofunknown/openacai-loader/tree/openacai-net11-preview
- OpenACAI BepInEx fork:
  https://github.com/desertofunknown/openacai-bepinex/tree/openacai-net11-preview
- OpenACAI UnityDoorstop fork:
  https://github.com/desertofunknown/openacai-unitydoorstop/tree/openacai-hostfxr-net10
- OpenACAI RedLoader compatibility source:
  https://github.com/desertofunknown/openacai-redloader/tree/openacai-loader-compat
- OpenACAI MonoMod fork:
  https://github.com/desertofunknown/openacai-monomod/tree/openacai-net11-preview

Upstream source provenance:

- RedManager by ToniMacaroni:
  https://github.com/ToniMacaroni/RedManager
- RedLoader by ToniMacaroni, rewrite branch:
  https://github.com/ToniMacaroni/RedLoader/tree/rewrite
- BepInEx:
  https://github.com/BepInEx/BepInEx
- UnityDoorstop:
  https://github.com/NeighTools/UnityDoorstop
- MonoMod:
  https://github.com/MonoMod/MonoMod

## Licenses And Credits

The OpenACAI Mod Manager and OpenACAI Endnight Loader source are licensed as
GPL-3.0-or-later where OpenACAI-authored changes are separably licensable.
Third-party components keep their original licenses. See the repository
`LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES`, and `LICENSES` folders/files for
component-specific terms.

Endnight Games UI texture/style assets, where bundled, are included only for
this open-source Sons Of The Forest modding tool with permission. They are
copyright property of Endnight Games and are not available for commercial use.

## Build Environment

The current Windows build environment used by OpenACAI is:

- Windows 10 or Windows 11
- Git
- PowerShell 7 or Windows PowerShell
- Visual Studio 2022 Build Tools with the Desktop C++ workload and Windows SDK
- WebView2 Runtime
- Node.js `24.16.0`
- npm `11.16.0`
- Rust/Cargo `1.96.0`
- .NET SDK `11.0.100-preview.4.26230.115`
- .NET SDK `8.0.x` and `6.0.x` are useful for transitional upstream/forked
  projects that still contain older target frameworks
- 7-Zip `26.01` or newer is recommended for maximum ZIP compression

The manager can be built without the game installed. A complete loader package
requires a legal Sons Of The Forest installation because the build references
generated IL2CPP interop assemblies and game assemblies produced from that
installation.

## Recommended Source Layout

The default loader build script expects this workspace layout:

```text
ModdingWorkspace/
  OpenACAILoaderSuite/
  RedLoader/
  Forks/
    BepInEx/
    UnityDoorstop/
    MonoMod/
```

If a different layout is used, pass explicit paths to the loader build script
where supported.

## Build The Mod Manager

From the manager repository:

```powershell
cd "C:\path\to\openacai-mod-manager"
npm ci
npm run check
npm run build
npm run tauri build
```

Expected unsigned build outputs:

```text
src-tauri/target/release/OpenACAI Mod Manager.exe
src-tauri/target/release/bundle/nsis/OpenACAI Mod Manager_0.2.0_x64-setup.exe
```

To copy the release binaries into the repository `prebuilt` folder and generate
hashes:

```powershell
.\scripts\refresh-prebuilt.ps1 -Build
```

Expected prebuilt outputs:

```text
prebuilt/OpenACAI-Mod-Manager-0.2.0-x64-portable.exe
prebuilt/OpenACAI-Mod-Manager-0.2.0-x64-setup.exe
prebuilt/OpenACAI-Mod-Manager.ico
prebuilt/SHA256SUMS.txt
```

Code signing is optional for compilation and is done after the build:

```powershell
.\scripts\refresh-prebuilt.ps1 -Build -Sign
```

Signing requires the OpenACAI Azure Trusted Signing environment to be configured
locally. Reviewers do not need the signing credentials to compile or inspect the
source.

## Build Loader Dependencies

The loader package uses OpenACAI forks of BepInEx, UnityDoorstop, MonoMod, and
RedLoader compatibility assemblies. The loader repository includes
`build-openacai-loader.ps1`, which performs the coordinated package build from
the expected workspace layout.

If MonoMod packages are not already present in:

```text
Forks/MonoMod/artifacts/openacai-nuget/
```

build the OpenACAI MonoMod packages first:

```powershell
cd "C:\path\to\ModdingWorkspace\Forks\MonoMod"
dotnet pack -c Release -p:ContinuousIntegrationBuild=true -o artifacts\openacai-nuget src\MonoMod.Core\MonoMod.Core.csproj
dotnet pack -c Release -p:ContinuousIntegrationBuild=true -o artifacts\openacai-nuget src\MonoMod.RuntimeDetour\MonoMod.RuntimeDetour.csproj
dotnet pack -c Release -p:ContinuousIntegrationBuild=true -o artifacts\openacai-nuget src\MonoMod.Utils\MonoMod.Utils.csproj
```

Then restore/build the OpenACAI BepInEx fork:

```powershell
cd "C:\path\to\ModdingWorkspace\Forks\BepInEx"
dotnet restore
dotnet build .\BepInEx.sln -c Release
```

The RedLoader compatibility source currently provides support assemblies still
used by the transition package. Build the RedLoader/SonsSdk projects in Debug
configuration so the loader packaging script can find the expected outputs:

```powershell
cd "C:\path\to\ModdingWorkspace\RedLoader"
dotnet build .\SonsSdk\SonsSdk.csproj -c Debug
dotnet build .\RedLoader\RedLoader.csproj -c Debug
```

Expected RedLoader support outputs:

```text
RedLoader/SonsSdk/bin/Debug/net6/SonsSdk.dll
RedLoader/SonsSdk/bin/Debug/net6/RedLoader.dll
```

The UnityDoorstop fork can be built by the loader script when xmake is
available. If building it manually:

```powershell
cd "C:\path\to\ModdingWorkspace\Forks\UnityDoorstop"
xmake config --arch=x64 --mode=release
xmake build
```

## Build The Loader Package

The loader branch is currently a .NET 11 preview package. It intentionally
rejects older runtime-major settings on this branch.

From the loader repository:

```powershell
cd "C:\path\to\ModdingWorkspace\OpenACAILoaderSuite"
.\build-openacai-loader.ps1 -GameRoot "C:\Program Files (x86)\Steam\steamapps\common\Sons Of The Forest"
```

Expected outputs:

```text
dist/OpenACAILoader.zip
dist/OpenACAILoader.manifest.json
dist/OpenACAILoader.zip.sha256
```

The script builds:

- `src/OpenACAI.Endnight.Loader.Core/OpenACAI.Endnight.Loader.Core.csproj`
- the OpenACAI BepInEx IL2CPP runtime pieces
- the OpenACAI UnityDoorstop entrypoint when available
- the transitional RedLoader/SonsSdk compatibility assemblies

The package includes loader files, managed dependencies, runtime files,
manifests, notices, and third-party license material.

## Game Interop Requirement

Sons Of The Forest is an IL2CPP Unity game. A complete loader build references
generated interop assemblies such as:

- `Il2Cppmscorlib`
- `Sons`
- `Sons.Gui`
- `Sons.Physics`
- `TheForest.Utils`
- `UnityEngine.*`
- `Unity.TextMeshPro`

These are generated from a local legal game installation and are not treated as
public source code. Reviewers compiling the loader should use their own legal
Sons Of The Forest install and generate or provide equivalent interop assemblies
from that installation.

## Verify Built Artifacts

Manager:

```powershell
Get-FileHash ".\prebuilt\OpenACAI-Mod-Manager-0.2.0-x64-portable.exe" -Algorithm SHA256
Get-FileHash ".\prebuilt\OpenACAI-Mod-Manager-0.2.0-x64-setup.exe" -Algorithm SHA256
```

Loader:

```powershell
Get-FileHash ".\dist\OpenACAILoader.zip" -Algorithm SHA256
Get-Content ".\dist\OpenACAILoader.manifest.json"
```

The mod manager's install/repair flow also verifies loader file hashes from the
release manifest before reporting the loader as installed.
