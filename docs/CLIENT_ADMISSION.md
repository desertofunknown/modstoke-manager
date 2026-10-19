# Managed client participation

Modstoke Manager, by OpenACAI, can prepare an installed Modstoke Client to
participate in managed admission when Sons of the Forest next starts.

Select a player installation, open **Main → Managed client participation**, and
choose **Read setting**. **Enable for next start** opts in to managed admission;
**Disable for next start** restores ordinary vanilla joining and hosting. These
actions change only the Client preference. Starting the game, changing profiles
or importing a server requirements list never enables participation automatically.

The panel checks the expected Client manifest and assembly in
`OpenACAI/Mods/Modstoke.Client`. File presence does not establish successful
loading, an unambiguous mod inventory, compatibility or client integrity. Use
the Client's in-game admission view to inspect its current state and recent
attempts. Manager does not read live game state or confirm server acceptance.

## Configuration

The selected installation's `BepInEx/config/modstoke.client.cfg` contains:

```ini
# Participate in managed admission on the next game start.
EnableManagedAdmission = true
```

A missing file defaults to disabled. The file must be valid UTF-8, at most
1024 bytes, and contain exactly one `EnableManagedAdmission` setting with a
Boolean value. Blank lines and lines beginning with `#` after whitespace are
allowed. Unknown settings, duplicates and malformed values require manual
correction; Manager does not replace the file with defaults.

Manager preserves valid comments, whitespace, line endings and any UTF-8 byte
order mark when replacing the Boolean value. It checks that the file still
matches the bytes read earlier, serializes writes with other Manager operations
on that installation, and publishes a flushed temporary file from the same
directory. Vortex-owned configuration and linked paths are refused. A failed
or interrupted response requires reading the setting again before retrying.

## Before saving

Both actions require the expected Client files and a stopped game. On Windows,
Manager checks for processes named `SonsOfTheForest.exe` or
`SonsOfTheForestDS.exe`, including instances belonging to another installation.
An incomplete process check blocks saving. Reading the setting remains available.

The process check is an observation immediately before the write, not a lock
against a separate launcher starting the game. Keep the game and server closed
until saving finishes. A successful save applies to the next start; it cannot
change an existing network session. Dedicated-server admission policy is managed
separately by the server, and is not configured through this player control.

## Validation status

The configuration handler and interface have source and build checks. Ordinary
app inspection covers reading the installed Client and its saved preference.
Configuration writes, process-blocked saves and end-to-end managed joining still
need their own requested acceptance exercise. Mod reports are declarations, not
proof that a client is honest.
