# Native package installation

Modstoke Manager by OpenACAI can install a local native mod ZIP into a selected
player or dedicated-server installation. Open **Native packages**, select the
ZIP and review its identity, version, destination and required dependencies
before choosing Install. Close the game and dedicated server first.

For a collection with a shared recovery point, use [Portable native packs](NATIVE_PACKS.md).
That workflow prepares its archives, then offers a separate installation review.

Each ZIP contains one executable mod, including its dependencies as ordinary
assembly files where appropriate. Other native mods named in
`modstokeDependencies.required` must already be installed at their exact declared
versions. Manager checks the native dependency graph, including installed mods
that depend on the version being replaced. It does not download dependencies.
Runtime enforcement of this declaration requires Modstoke Loader SDK package
0103 or later; inspecting files does not establish which Loader is running.

Use this archive layout:

```text
OpenACAI/Mods/Example.Mod/
  openacai.mod.json
  Example.Mod.dll
  LICENSE
  config/defaults.ini
```

The package folder and mod IDs use 1–96 ASCII letters, digits, dots, underscores
or hyphens. The UTF-8 manifest explicitly declares `id`, `version`, a relative
DLL path and an entrypoint type. Supported aliases follow the native Loader;
Manager requires an unambiguous DLL path instead of choosing a fallback file.
Exactly one `openacai.mod.json`, `openacai.json` or `manifest.json` appears at the
package root. Disabled paths, nested manifests and library-only packages are
outside this installer format. Existing manually installed packages stay listed
without being converted to Manager ownership.

ZIPs are limited to 256 MiB, 8192 entries and 512 MiB of decoded files. Manifests
are limited to 512 KiB and 16 JSON container levels. Duplicate identities, paths
and JSON fields, unsupported dependency declarations and filesystem aliases
stop preparation. Exact versions are text comparisons, not version ranges.
Dependency planning requires readable, unambiguous enabled native manifests;
it does not validate executable entrypoint behavior or server compatibility.

Manager updates only native packages it previously installed at the same
destination. Existing `config` and `userdata` directories and `.cfg`/`.ini` files
are preserved. Manifests and executable files cannot live in those preserved
paths. Other changed, missing or unowned files stop the update. Unchanged files
owned by the previous version can be removed when absent from the new ZIP.
Vortex files and manually installed packages remain protected.

The existing `modstoke-profile.lock.json` records native provenance, exact
requirements, archive and payload hashes, and pending progress. Its new local
source uses `modstoke-native-local`; the existing `url` field stores a
`urn:sha256:` content identifier rather than a download address or local path.
Native and legacy packages share an ID namespace in this profile, so conflicting
IDs are rejected. Existing schema 1/2 records remain readable. Older Manager
versions reject the new provider or fields instead of silently rewriting them.

Interrupted installs retain their plan. Select the same ZIP under Native
packages to resume, or use **Pack recovery** to restore the previous files and
profile records. A portable pack uses **Load pending pack** and every original
ZIP to resume the collection. Its optional `pendingPlan.nativePack` descriptor
is retained until the plan finishes; use the current Manager to recover it.
The latest recovery point remains available until discarded
or replaced by another install. Changed files can prevent rollback; Manager
preserves those changes rather than overwriting them. Archive hashes remain in
the profile after rollback, preventing different bytes from silently replacing
an already recorded exact version.

This local installer does not execute mod code during inspection. Hashes bind
the reviewed ZIP to its staged files; they are not publisher signatures or proof
that a client is honest. Installing files into a dedicated-server profile does
not establish that the mod or Loader supports that server build.
