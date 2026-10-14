# Local Testing

Use the refresh script to copy a local Modstoke Manager build into the prebuilt and local test folders:

```powershell
.\scripts\refresh-prebuilt.ps1 -Build
```

The script copies the portable executable to:

```text
local-test\OpenACAI-Mod-Manager-0.2.0-x64-portable.exe
```

`local-test/` is ignored by Git so test-only binaries stay local.

The executable filename is retained for compatibility. Refreshing the prebuilts replaces the historical OpenACAI-branded binaries with your local build; it does not sign or publish them.
