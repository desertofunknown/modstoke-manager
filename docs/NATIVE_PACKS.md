# Portable native packs

Modstoke Manager by OpenACAI prepares shareable collections of native mod ZIPs.
Open **Native packages → Portable native packs**, enter a name and select the
ZIPs. Every declared native dependency must be included at its exact version.
Manager checks each archive and lists dependencies before the mods that need
them. Installed packages do not supply missing dependencies during preparation.

Save the pack descriptor and share it alongside the original ZIPs, subject to
their authors' distribution terms. The descriptor contains package identities,
versions, folder names, exact dependencies, archive sizes and SHA-256 hashes.
It contains no local filesystem paths, settings, saves or player information.
No mod code runs during preparation, and Manager does not download packages.

To use a descriptor received from someone else, import its JSON and choose
**Verify ZIPs**. Select all the original archives. Manager requires their hashes,
sizes and package metadata to match the descriptor, with no missing or extra
packages. An imported descriptor is unverified until those archives are checked.
Verification describes the files read at that time; changing them afterward
requires another inspection.

Preparation does not change the selected installation, its profile lockfile or
its recovery point. To install the verified collection, stop the game and
dedicated server, then choose **Review installation…**. Review the destination,
package actions and recovery notices before choosing **Install pack**.

Manager plans all selected versions together and checks the requirements of
other installed native mods. It snapshots and verifies every selected ZIP before
starting the plan. Existing configurations are preserved under the
[native package rules](NATIVE_PACKAGES.md). Manually installed or externally
managed targets are not adopted or overwritten.

Packages are applied in dependency order under one recovery point named after
the pack. Files change sequentially; an interruption or failure can leave some
packages applied and others pending. Keep the game and server stopped until
the plan is finished or rolled back. Choose **Load pending pack**, select all
original ZIPs, verify and review them, then choose **Resume pack**. Include ZIPs
for already-applied packages; Manager checks their recorded files without
applying those steps again.

Manager's player and server launch actions refuse unfinished plans or recovery.
Launching through Steam or another program directly bypasses that check.

**Pack recovery** restores the plan's originals in reverse package order and
restores the prior profile records. Conflicting external changes stop recovery
and remain available for inspection. Starting a new pack replaces the previous
completed recovery point; resuming an unfinished pack retains its checkpoint.
Installing ZIPs separately still creates separate recovery points.

The pending profile stores the descriptor in `pendingPlan.nativePack`. Existing
schema 1/2 profiles without that optional field remain readable. Older Managers
reject the field while the pack is pending, so use the current Manager to finish
or recover it; do not edit or delete the lockfile to bypass recovery. Completion
clears the pending plan. Completed package records keep the existing native
provider and `urn:sha256:` provenance, and archive hashes remain recorded even
after rollback.

## Format and limits

The portable descriptor uses `format: "modstoke.native-pack"` and
`schemaVersion: 1`. Its only other top-level fields are `name` and `packages`.
Each package contains `id`, `name`, `version`, `folder`, `required`, `sha256`
and `size`; each required dependency contains `id` and `version`. Size is the
ZIP's length in bytes, and hashes use lowercase hexadecimal. The folder is the
single directory directly beneath `OpenACAI/Mods`.

The descriptor is limited to 256 KiB and 128 packages. Pack names contain
1–120 Unicode characters without surrounding whitespace or control characters.
Each package may require up to 64 other packages. IDs and folders are compared
without ASCII letter case; versions are exact text, with no ranges. Duplicate
IDs, conflicting folders, missing dependencies, wrong versions and dependency
cycles stop preparation. Unknown or duplicate JSON fields are rejected.

Each ZIP follows the [native package rules](NATIVE_PACKAGES.md), including the
256 MiB archive, 512 MiB decoded size and 8192-entry limits. A prepared collection
is limited to 2 GiB of archives, 2 GiB of decoded files and 32,768 files. These
limits apply before installation and do not reserve space on a destination disk.
Installation also uses the existing profile limits: 4 MiB of records, 128 steps
and 1,024 dependency links. Profile and recovery metadata can therefore limit
an installation that fits the portable descriptor limits.

This format is separate from the installation-bound `modstoke-profile.lock.json`
and from [server requirements](SERVER_REQUIREMENTS.md). Importing or verifying a
pack does not activate admission policy. Archive hashes establish equality of
selected bytes; they do not authenticate a publisher, prove successful loading
or establish that a participating client is honest. Choose packages from trusted
authors and qualify their player or dedicated-server behavior separately.

Pack import, installation, interrupted resume and rollback still need app
acceptance with real archives. Compilation and layout checks do not establish
that recovery works after a process interruption.
