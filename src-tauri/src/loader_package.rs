use serde::Serialize;
use std::fs::File;
use zip::read::ZipArchive;

#[derive(Serialize)]
pub struct LoaderZipInspection {
    is_valid: bool,
    has_manifest: bool,
    has_bridge: bool,
    has_bepinex_core: bool,
    has_doorstop: bool,
    has_private_admin_tool: bool,
    entries: usize,
    errors: Vec<String>,
}

fn normalize_zip_entry(name: &str) -> String {
    name.replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

#[tauri::command(async)]
pub fn inspect_openacai_loader_zip(source: String) -> Result<LoaderZipInspection, String> {
    let reader = File::open(source).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(reader).map_err(|e| e.to_string())?;
    let mut inspection = LoaderZipInspection {
        is_valid: false,
        has_manifest: false,
        has_bridge: false,
        has_bepinex_core: false,
        has_doorstop: false,
        has_private_admin_tool: false,
        entries: archive.len(),
        errors: Vec::new(),
    };

    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| e.to_string())?;
        if file.enclosed_name().is_none() {
            inspection
                .errors
                .push(format!("Unsafe path in zip entry: {}", file.name()));
            continue;
        }

        let entry = normalize_zip_entry(file.name());
        inspection.has_manifest |=
            entry == "bepinex/plugins/openacailoader/openacai-loader.manifest.json";
        // Packages built before the Modstoke rename ship the core under its old assembly name.
        inspection.has_bridge |= entry == "bepinex/plugins/openacailoader/modstoke.loader.dll"
            || entry == "bepinex/plugins/openacailoader/openacai.endnight.loader.core.dll";
        inspection.has_bepinex_core |= entry == "bepinex/core/bepinex.core.dll";
        inspection.has_doorstop |= entry == "winhttp.dll" || entry == "doorstop_config.ini";
        inspection.has_private_admin_tool |= entry.contains("openacaiadmintool")
            || entry.contains("anti-cheat")
            || entry.contains("anticheat");
    }

    if !inspection.has_manifest {
        inspection.errors.push(
            "Missing BepInEx/plugins/OpenACAILoader/openacai-loader.manifest.json".to_string(),
        );
    }

    if !inspection.has_bridge {
        inspection.errors.push(
            "Missing Modstoke Loader core assembly at BepInEx/plugins/OpenACAILoader/Modstoke.Loader.dll".to_string(),
        );
    }

    if !inspection.has_bepinex_core {
        inspection
            .errors
            .push("Missing BepInEx/core/BepInEx.Core.dll".to_string());
    }

    if !inspection.has_doorstop {
        inspection.errors.push(
            "Missing Doorstop bootstrap files (winhttp.dll or doorstop_config.ini).".to_string(),
        );
    }

    if inspection.has_private_admin_tool {
        inspection.errors.push("Package appears to contain private Modstoke Admin/anti-cheat files; public loader installs must not include them.".to_string());
    }

    inspection.is_valid = inspection.errors.is_empty();
    Ok(inspection)
}
