# Dedicated servers

Modstoke Manager, by OpenACAI, can remember a local Sons of the Forest dedicated
server's installation and data folder, inspect its save settings and start its
executable. It does not install the dedicated server or configure its network.

## Prepare the server

Install and configure the game's dedicated server through its normal setup.
Follow the [dedicated-server configuration guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2992700419).
Keep the server installation separate from the player installation.

Open **Dedicated servers** in Manager, add `SonsOfTheForestDS.exe`, select that installation and choose
**Read status**. **Choose data folder** selects an existing folder
containing `dedicatedserver.cfg`. This saves a Manager preference; it does not
move files or change the native configuration, owner list or saves.

Manager shows the server name, save mode, save slot and configured player limit.
The player limit is configuration information, not a qualified capacity claim.
Passwords and the rest of the configuration are not displayed.

## Change the server name

Read status, enter a name under **Server name**, then review and save the change.
Names must contain 1–256 characters without control characters. Manager trims
surrounding spaces. The server and player game must be stopped. The change keeps
all other configuration values and uses the same original-file backup and revision
checks as a player-limit change. Read status again after saving.

## Change the player limit

Close the player game and stop the dedicated server, then read status. Under
**Player limit**, choose 8, 16 or 32, select **Review limit**,
and confirm the change. Limits above 8 require an explicit acknowledgement:
they need compatible Modstoke Loader support on the server and remain experimental.
Saving a number does not install Loader, start the server or establish that the
chosen population works in multiplayer. Qualification proceeds at 8, 16 and 32
players, including admission, rosters, reconnects, saves and replication cost.

Manager changes only the `MaxPlayers` value, adding the property if it is absent.
It preserves other configuration bytes, including unknown properties, formatting
and the UTF-8 byte-order mark. An existing limit outside these choices remains
unchanged until you explicitly select a new value. The selected installation,
data folder and original configuration must still match the reviewed observation.
Pending package or world recovery blocks the change.

Before replacing the configuration, Manager saves and verifies an exact copy in
`.modstoke-config-backups` inside the data folder. The filename contains the original
configuration's SHA-256 hash. Up to 64 entries are retained without automatic pruning;
an identical existing backup can be reused. These are complete native configuration
files and can contain passwords, so keep them private. To restore one manually, stop
the game and server, preserve the current file, and copy the chosen backup to
`dedicatedserver.cfg`. Read status again afterward.

The write replaces one file after its backup has been saved. If Manager closes or
reports an uncertain result, read the current configuration before trying again;
it may already contain the new limit. Manager does not retry or restore automatically.
External editors and processes can still act independently of Manager's locks.

## Start and observe

Use **Start server** after reading the configuration summary. Manager
starts the selected executable in its installation folder, passing the chosen
folder with the game's `-userdatapath` argument. It does not invoke the player
Steam launch, append custom arguments or run a batch file.

Launch requires `SaveMode` to be `Continue`, including the native default when
the field is absent. `New` overwrites the selected save slot on each native
server start, so Manager blocks that mode instead of changing it. A missing,
unsupported or unreadable configuration also blocks launch. Manager accepts
UTF-8 JSON up to 64 KiB with ASCII top-level property names, and rejects duplicate
top-level properties, comments and trailing commas. Configure or correct the native file outside Manager, then
read it again.

A changed data-folder preference or configuration requires another read before
launch. An unfinished Manager package plan, update transaction or pack recovery
also blocks launch. The launch action never performs package recovery itself.

Status reports the process observed at the last refresh. A running process does
not establish that startup checks passed, clients can join, mods loaded or the
world is ready. Manager records the exit code of the process it started; this
is not confirmation that a save completed. Refresh status to observe changes.

Manager does not stop or automatically restart the server. Use the server's
supported shutdown controls before maintenance. Closing Manager leaves the
server running. After Manager restarts, an existing process is reported as
external; ownership is not inferred from its process ID.

## World backup and recovery

Close the player game and stop the dedicated server through its normal controls
before using **World backups** in the server workspace. Read its status, choose the existing native save folder
inside the configured data folder, then read status again. The folder must
contain `SaveData.zip` or the loose `SaveData.json`. Manager does not infer the
dedicated server's save directory from `SaveSlot`; select the actual world you
intend to preserve.

**Create complete backup** copies every file and directory in that folder,
including loose fallback files. Modstoke's current world records are stored
inside the native save and travel with it. Manager checks the complete file
inventory and contents before publishing the backup. This preserves the bytes
present while stopped; it does not establish that the game's last save completed
successfully or that the world will load.

Choose a backup, use **Review restore**, and confirm the selected installation
and save folder. Restore stages the complete saved folder and retains an undo
backup of the current one before replacing it. Backups are bound to the same
installation, data folder and selected save folder. Server configuration,
installed mods and other world folders are outside the restore.

If Manager stops during a restore, use **Read pending recovery** after reopening
it. This also works when the save folder is temporarily absent between directory
moves. **Review finish** continues the recorded restore; **Review undo** returns
the previous folder. Each requires confirmation. Changed or unrecognized files
halt recovery and remain preserved. Manager blocks server launch and data-folder
changes while a restore is pending; it never retries recovery automatically.

Backups and retained restore work are local to `.modstoke-world-backups` in the
configured data folder. A snapshot supports up to 4,096 files and 4,096 directories,
4 GiB total, 1 GiB per file and 16 directory levels. Manager retains at most 64
backups and 64 workspaces of each kind, without automatic pruning. Restoring needs
additional disk space for the staged world, undo backup and displaced folder.
Keep copies outside the server's disk for protection against disk failure.

The current restore implementation requires Windows. These backups include only
the selected native folder. Older mods may keep sidecar records elsewhere; those
need separate recovery and must not be assumed to match the restored world.
External tools can still edit files or start a server independently of Manager's
locks. Backup, interruption recovery and native reload require runtime acceptance.

## Package files and recovery

On Main, use **Select ZIP…** under Native packages to inspect a local native mod
archive for the selected server installation. Review its identity, version,
installation root, destination and required dependencies before choosing
**Install package**, **Update package** or **Resume package**. Close the game and
dedicated server first. Required dependencies must already be installed at their
exact declared versions; updates apply only to packages previously installed by
Manager. Select the same ZIP to resume an interrupted install, or use
**Pack recovery** to restore its previous files and profile records. See
[native package installation](NATIVE_PACKAGES.md) for the archive format and
configuration preservation rules.

Native inventory is a filesystem observation, not proof that a package loaded or
supports the dedicated server. Legacy catalog installation, toggling and removal
remain unavailable for server profiles. Dedicated-server launch and native package
installation, update and recovery still need app acceptance.

While Manager owns a running server, it retains locks for the installation and
selected data folder. It releases them when it observes the child exit, or when
Manager exits. Reading status or requesting another operation observes that
exit. All Manager operations that take a server installation's update lock also
check for running dedicated-server processes. Unknown process status blocks
changes. These checks use the standard executable name and are snapshots;
external tools can still launch or modify the server independently.

Manager currently allows one dedicated-server process at a time. A detected
server in another installation also blocks launch and data-folder changes.
The player game may remain open. Server inventory can still be inspected while
recovery is blocked. Refresh its inventory after shutdown to recover recognized
interrupted Manager updates. Conflicting files or unfinished plans need their
reported recovery steps resolved before launch.

Data-folder preferences use separate schema-1 files under `server-launch` in
Manager's existing app configuration folder. Installation registry and package
lockfile formats remain unchanged. Re-adding a forgotten installation reuses
its saved data-folder choice; forgetting an installation never deletes data.
Remote server control and graceful shutdown integration remain separate work.
