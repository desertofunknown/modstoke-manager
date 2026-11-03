use crate::hashing::sha256_reader;
use std::ffi::OsStr;
use std::path::{Component, Path, Prefix};
use std::fs::File;
use windows::{
    core::PCWSTR,
    Win32::{Foundation::HWND, UI::Shell::ShellExecuteW},
};
use winreg::{enums::*, RegKey};

#[tauri::command(async)]
pub fn is_dotnet11_installed() -> Result<bool, String> {
    is_dotnet_major_installed("11.")
}

fn is_dotnet_major_installed(version_prefix: &str) -> Result<bool, String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey_with_flags(
        r"SOFTWARE\WOW6432Node\dotnet\Setup\InstalledVersions\x64\sharedfx\Microsoft.NETCore.App", 
        KEY_READ
    ).map_err(|op| op.to_string())?;

    let contains_runtime = key
        .enum_values()
        .filter_map(Result::ok)
        .any(|(name, _)| name.starts_with(version_prefix));

    Ok(contains_runtime)
}

#[tauri::command(async)]
pub fn sha256_file(path: String) -> Result<String, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    sha256_reader(file).map_err(|e| e.to_string())
}

// Call this from a plain (not async) command. Those run on the main thread, where COM is already set up
// for the shell.
pub fn shell_execute(verb: &str, target: &OsStr) -> Result<(), ()> {
    const SW_SHOWNORMAL: i32 = 1;
    let result = unsafe { ShellExecuteW(HWND::default(), verb, target, PCWSTR::default(), PCWSTR::default(), SW_SHOWNORMAL) };
    // Values above 32 mean success.
    if result.0 > 32 { Ok(()) } else { Err(()) }
}

// Only an existing folder on a local or mapped drive is opened. The explore verb has no meaning
// for a file, so a path swapped for a program at the last moment is refused instead of run.
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let folder = Path::new(&path);
    let on_drive = matches!(folder.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)));
    if !on_drive || !folder.is_absolute() || !folder.is_dir() {
        return Err("This folder is unavailable.".into());
    }
    shell_execute("explore", folder.as_os_str()).map_err(|_| "Windows Explorer could not open this folder.".into())
}
