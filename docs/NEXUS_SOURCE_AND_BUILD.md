# Nexus Source And Build Instructions

This document is for Nexus Mods review of Modstoke Manager, by OpenACAI, and the
Modstoke Loader package it installs for Sons Of The Forest.

For the Manager's API behavior, account handling and pending OAuth registration,
start with the [Nexus integration notes](NEXUS_API_REVIEW.md). The signed 0.2.0
downloads predate these changes; build the current Manager branch for review.

The project is open source. The Manager builds from this repository alone. It installs and verifies a loader package, but the loader source is published only as a partial snapshot for review, so the loader steps below describe the maintainer's local build and can't yet be repeated from public source.

## Public Source Links

OpenACAI source repositories:

- Modstoke Manager:
  https://github.com/desertofunknown/modstoke-manager/tree/openacai-tauri2-svelte5-shell
- Modstoke Loader (partial source snapshot for review, not a working build):
  https://github.com/desertofunknown/modstoke-loader/tree/loader-source-preview
- OpenACAI BepInEx and RedLoader fork changes: the `source-snapshot/upstream-forks`
  folder on that branch

The OpenACAI UnityDoorstop and MonoMod forks are not published yet.

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

The Modstoke Manager and Modstoke Loader source are licensed as
GPL-3.0-or-later where OpenACAI-authored changes are separably licensable.
Third-party components keep their original licenses. See the repository
`LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md` and `LICENSES/` files for
component-specific terms.

The source contains no Endnight Games assets, so builds made from it carry
none. The Sons of the Forest skin reads its textures and font from the player's
own game installation at runtime.

## Build Environment

The Windows build tools are:

- Windows 10 or Windows 11
- Git
- PowerShell 7 or Windows PowerShell
- Visual Studio 2022 Build Tools with the Desktop C++ workload and Windows SDK
- WebView2 Runtime
- Node.js `24.16.0`
- npm `11.16.0`
- Rust/Cargo `1.96.0`
- .NET SDK `11.0.100-rc.2.26504.105` for new loader builds
- .NET SDK `8.0.x` and `6.0.x` are useful for transitional upstream/forked
  projects that still contain older target frameworks
- 7-Zip `26.01` or newer is recommended for maximum ZIP compression

The manager can be built without the game installed. A complete loader package
requires a legal Sons Of The Forest installation because the build references
generated IL2CPP interop assemblies and game assemblies produced from that
installation.

The manager itself uses Rust and Svelte; it does not require a .NET SDK to
build. Its runtime detection already accepts .NET 11 RC2, and loader update
metadata supplies the runtime version shown in the manager.

## Recommended Source Layout

Existing repositories, package files, runtime executable, namespaces, and
loader build paths retain their compatibility names. The source-built installer
uses the Modstoke Manager name. Signed `0.2.0` downloads retain the OpenACAI Mod
Manager name and branding.

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
cd "C:\path\to\modstoke-manager"
npm ci
npm run check
npm run build
npm run tauri build
```

Expected unsigned build outputs:

```text
src-tauri/target/release/OpenACAI Mod Manager.exe
src-tauri/target/release/bundle/nsis/Modstoke Manager_0.2.0_x64-setup.exe
```

`scripts\refresh-prebuilt.ps1` deletes the Manager files in `prebuilt/`, copies in the new build and rewrites `SHA256SUMS.txt`. Only signed builds go in `prebuilt/` or on a release, so always run it with `-Sign` (see below). Newer signed builds aren't available yet, so `prebuilt/` keeps the signed 0.2.0 files for now. When the version changes, pass it with `-Version`; the script defaults to 0.2.0.

Expected prebuilt outputs:

```text
prebuilt/OpenACAI-Mod-Manager-0.2.0-x64-portable.exe
prebuilt/OpenACAI-Mod-Manager-0.2.0-x64-setup.exe
prebuilt/OpenACAI-Mod-Manager.ico
prebuilt/SHA256SUMS.txt
```

Signing is not needed to compile or inspect the source, but every binary committed to `prebuilt/` or attached to a release must be signed:

```powershell
.\scripts\refresh-prebuilt.ps1 -Build -Sign
```

This signs the portable exe and the installer after the build. It does not sign the copy of the app inside the installer, so the installed `OpenACAI Mod Manager.exe` stays unsigned (the signed 0.2.0 installer included) until the build signs the app before packaging it. If the build or signing stops partway, `prebuilt/` is left with missing or unsigned files: run `git restore prebuilt` and don't commit it. Check every file with `Get-AuthenticodeSignature` before committing. Signing needs OpenACAI's Azure Artifact Signing account or its certificate; see [SIGNING.md](../SIGNING.md).

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

The loader targets `net11.0` and rejects older runtime-major settings.

For the RC2 upgrade, use SDK `11.0.100-rc.2.26504.105` and runtime
`11.0.0-rc.2.26504.105`, released on October 13, 2026. See Microsoft's
[RC2 announcement](https://devblogs.microsoft.com/dotnet/dotnet-11-rc-2/) and
[downloads](https://dotnet.microsoft.com/en-us/download/dotnet/11.0).

The existing loader package still bundles Preview 4. Installing an SDK on the
development machine does not upgrade that package or an installed game's
runtime. In the loader repository:

1. Update the SDK version in `global.json` to `11.0.100-rc.2.26504.105`.
2. Build using a .NET root containing the RC2 SDK, runtime and hostfxr. Pass
   that directory through `-DotnetRoot` to avoid packaging an older runtime
   from another installation. Put its `dotnet.exe` on `PATH` as well: the
   script invokes `dotnet build` from `PATH`, while `-DotnetRoot` selects the
   runtime and hostfxr to bundle.
3. Regenerate the loader ZIP, manifest and checksum together. Confirm the
   manifest's `runtime.dotnetRuntimeVersion` is `11.0.0-rc.2.26504.105` before
   publishing the package to the loader release feed at modstoke.com, which the
   Manager reads. No release has been published there yet.

Keep the existing loader paths, assembly names and release asset names. The
manager reads the package version and runtime metadata from the release
manifest; it does not need a new hardcoded runtime pin.

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
Get-AuthenticodeSignature ".\prebuilt\OpenACAI-Mod-Manager-0.2.0-x64-portable.exe"
Get-AuthenticodeSignature ".\prebuilt\OpenACAI-Mod-Manager-0.2.0-x64-setup.exe"
```

The hashes should match `prebuilt/SHA256SUMS.txt`, and both files should report `Valid` with the signer OpenACAI Inc.

Loader:

```powershell
Get-FileHash ".\dist\OpenACAILoader.zip" -Algorithm SHA256
Get-Content ".\dist\OpenACAILoader.manifest.json"
```

The mod manager's install/repair flow also verifies loader file hashes from the
release manifest before reporting the loader as installed. That flow needs a
loader release at modstoke.com, which has not been published yet.
