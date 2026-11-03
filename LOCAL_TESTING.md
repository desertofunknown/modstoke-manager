# Local Testing

For everyday testing, run `npm run tauri dev`, or build with `npm run tauri build` and run `src-tauri\target\release\OpenACAI Mod Manager.exe`. Neither touches `prebuilt/`.

To test the files as they would be published, use the refresh script. It builds the app and copies it into `prebuilt/` and `local-test/`:

```powershell
.\scripts\refresh-prebuilt.ps1 -Build
```

The script copies the portable executable to:

```text
local-test\OpenACAI-Mod-Manager-0.2.0-x64-portable.exe
```

`local-test/` is ignored by Git so test-only binaries stay local.

The file names keep the old OpenACAI names for compatibility. Without `-Sign`, the script replaces the signed v0.2.0 files in `prebuilt/` with your unsigned build and rewrites `prebuilt/SHA256SUMS.txt`. Git tracks these files, so never commit or upload them. Run `git restore prebuilt` before committing. Signed builds are covered in [code signing](SIGNING.md).
