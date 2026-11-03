use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use winreg::{enums::*, RegKey};

pub const SOTF_APP_ID: &str = "1326470";

#[tauri::command]
pub async fn get_steam_path() -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let steam_install: PathBuf = hklm
        .open_subkey_with_flags(r"SOFTWARE\WOW6432Node\Valve\Steam", KEY_READ)
        .and_then(|key| key.get_value::<String, _>("InstallPath"))
        .or_else(|_| {
            hklm.open_subkey_with_flags(r"SOFTWARE\Valve\Steam", KEY_READ)
                .and_then(|key| key.get_value::<String, _>("InstallPath"))
        })
        .ok()?
        .into();

    let vdf = steam_install.join(r"steamapps\libraryfolders.vdf");
    if !vdf.exists() {
        return None;
    }

    let re = Regex::new(r#"\s"(?:\d|path)"\s+"(.+)""#).unwrap();
    let mut steam_paths = vec![steam_install.join(r"steamapps")];

    let file = File::open(vdf).ok()?;
    let reader = BufReader::new(file);
    for text in reader.lines().map_while(Result::ok) {
        if let Some(cap) = re.captures(&text) {
            steam_paths.push(PathBuf::from(cap[1].replace(r"\\", r"\")).join(r"steamapps"));
        }
    }

    let install_re = Regex::new(r#"\s"installdir"\s+"(.+)""#).unwrap();
    for path in steam_paths {
        let manifest = path.join(format!(r"appmanifest_{}.acf", SOTF_APP_ID));
        // One unreadable library must not hide the game in another.
        let Ok(file) = File::open(manifest) else { continue };
        for text in BufReader::new(file).lines().map_while(Result::ok) {
            if let Some(cap) = install_re.captures(&text) {
                let file_path = path.join("common").join(&cap[1]).join("SonsOfTheForest.exe");
                if let (true, Some(found)) = (file_path.exists(), file_path.to_str()) {
                    return Some(found.to_string());
                }
            }
        }
    }

    None
}

pub fn steam_executable() -> Result<PathBuf, String> {
    let current = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(r"Software\Valve\Steam", KEY_READ);
    let executable = current.as_ref().ok().and_then(|key| key.get_value::<String, _>("SteamExe").ok())
        .map(PathBuf::from)
        .or_else(|| current.as_ref().ok().and_then(|key| key.get_value::<String, _>("SteamPath").ok())
            .map(|path| Path::new(&path).join("steam.exe")))
        .or_else(|| {
            let machine = RegKey::predef(HKEY_LOCAL_MACHINE);
            [r"SOFTWARE\WOW6432Node\Valve\Steam", r"SOFTWARE\Valve\Steam"].iter().find_map(|path| {
                machine.open_subkey_with_flags(path, KEY_READ).ok()
                    .and_then(|key| key.get_value::<String, _>("InstallPath").ok())
                    .map(|path| Path::new(&path).join("steam.exe"))
            })
        }).ok_or("Steam could not be located. Open Steam and try again.")?;
    if !executable.is_absolute() || !executable.is_file()
        || !executable.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("steam.exe")) {
        return Err("The registered Steam executable is unavailable.".into());
    }
    Ok(executable)
}

// Steam starts itself first when it isn't running, then the game with its usual launch options.
#[tauri::command(async)]
pub fn launch_game_through_steam() -> Result<(), String> {
    std::process::Command::new(steam_executable()?)
        .args(["-applaunch", SOTF_APP_ID])
        .spawn()
        .map(drop)
        .map_err(|_| "Steam could not be started. Open Steam and try again.".into())
}
