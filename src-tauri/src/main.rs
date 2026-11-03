// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod admin_permissions;
mod client_admission;
mod downloads;
mod game_skin;
mod hashing;
mod installation_profiles;
mod loader_console;
mod loader_package;
mod mod_pack;
mod mod_profile;
mod mod_update;
mod native_inventory;
mod nexus;
mod safe_fs;
mod server_capacity;
mod server_console;
mod server_directory;
mod server_game_settings;
mod server_launch;
mod server_native_config;
mod server_process;
mod server_requirements;
mod server_world;
mod server_world_restore;
mod steam;
mod system;

use tauri::Manager;

// Only archives the Manager downloaded itself go through here; local packages have their own commands.
#[tauri::command(async)]
fn unzip_handler(app: tauri::AppHandle, source: String, destination: String) -> Result<(), String> {
    let archive = std::fs::canonicalize(&source).map_err(|_| "The downloaded archive is unavailable.")?;
    let downloads = std::fs::canonicalize(downloads::folder(&app)?).map_err(|error| error.to_string())?;
    if archive.parent() != Some(downloads.as_path()) || !archive.is_file() {
        return Err("Only archives the Manager downloaded can be installed this way.".into());
    }
    mod_update::install_zip_archive(archive.to_string_lossy().into_owned(), destination)
}

// The window starts hidden. It is sized for its monitor before it appears and shown once the page has
// loaded, so it never sits empty or jumps to a new size.
fn fit_to_monitor(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) else {
        return Ok(());
    };
    let area = monitor.work_area().size.to_logical::<f64>(monitor.scale_factor());
    let min_width = 980f64.min((area.width - 24.0).max(760.0));
    let min_height = 680f64.min((area.height - 24.0).max(600.0));
    let max_width = min_width.max(1920f64.min(area.width - 16.0));
    let max_height = min_height.max(1240f64.min(area.height - 16.0));
    window.set_min_size(Some(tauri::LogicalSize::new(min_width.round(), min_height.round())))?;
    window.set_size(tauri::LogicalSize::new(
        (area.width * 0.9).clamp(min_width, max_width).round(),
        (area.height * 0.94).clamp(min_height, max_height).round(),
    ))?;
    window.center()
}

fn main() {
    // Libraries loaded later by name follow the same rule as the exe's own imports.
    unsafe {
        use windows::Win32::System::LibraryLoader::{SetDefaultDllDirectories, LOAD_LIBRARY_FLAGS, LOAD_LIBRARY_SEARCH_SYSTEM32, LOAD_LIBRARY_SEARCH_USER_DIRS};
        SetDefaultDllDirectories(LOAD_LIBRARY_FLAGS(LOAD_LIBRARY_SEARCH_SYSTEM32.0 | LOAD_LIBRARY_SEARCH_USER_DIRS.0));
    }
    tauri::Builder::default()
        .manage(server_directory::PendingJoin::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            server_directory::receive_arguments(app, &args);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let args: Vec<String> = std::env::args_os().map(|arg| arg.to_string_lossy().into_owned()).collect();
            server_directory::receive_arguments(app.handle(), &args);
            if let Some(window) = app.get_webview_window("main") {
                let _ = fit_to_monitor(&window);
                // Shown anyway if the page never reports that it loaded.
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    if !window.is_visible().unwrap_or(true) {
                        let _ = window.show();
                    }
                });
            }
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = webview.window().show();
            }
        })
        .register_asynchronous_uri_scheme_protocol("gameskin", game_skin::serve)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            server_directory::list_community_servers,
            server_directory::get_community_server,
            server_directory::get_community_server_history,
            server_directory::take_pending_community_join,
            server_directory::join_community_server,
            unzip_handler,
            downloads::download_to_cache,
            nexus::open_nxm_link,
            steam::launch_game_through_steam,
            system::open_folder,
            mod_profile::read_mod_profile,
            mod_profile::check_player_launch_ready,
            mod_profile::write_mod_profile,
            mod_profile::apply_mod_profile_package,
            mod_profile::rollback_mod_profile,
            mod_profile::discard_mod_profile_rollback,
            mod_profile::native_local::inspect_native_package,
            mod_profile::native_local::install_native_package,
            mod_profile::native_pack::parse_native_pack,
            mod_profile::native_pack::prepare_native_pack,
            mod_profile::native_pack_install::inspect_native_pack_install,
            mod_profile::native_pack_install::install_native_pack,
            installation_profiles::read_installation_profiles,
            installation_profiles::inspect_installation_target,
            installation_profiles::write_installation_profiles,
            native_inventory::read_native_mod_inventory,
            server_requirements::parse_server_requirements,
            client_admission::read_client_admission,
            client_admission::write_client_admission,
            admin_permissions::read_admin_permissions,
            admin_permissions::check_admin_permissions,
            admin_permissions::write_admin_permissions,
            loader_console::read_loader_console,
            loader_console::submit_loader_console,
            game_skin::read_game_skin,
            server_launch::read_server_launch,
            server_launch::write_server_launch,
            server_launch::write_server_capacity,
            server_launch::write_server_identity,
            server_launch::write_server_log_files,
            server_launch::write_server_game_settings,
            server_game_settings::read_server_game_settings,
            server_launch::launch_dedicated_server,
            server_launch::stop_dedicated_server,
            server_console::read_server_console,
            server_world::read_server_world,
            server_world::create_server_world_backup,
            server_world_restore::restore_server_world_backup,
            server_world_restore::recover_server_world_restore,
            loader_package::inspect_openacai_loader_zip,
            steam::get_steam_path,
            system::is_dotnet11_installed,
            system::get_file_version,
            nexus::nexus_save_api_key,
            nexus::nexus_get_session,
            nexus::nexus_clear_api_key,
            nexus::nexus_fetch_sotf_mods,
            nexus::nexus_fetch_sotf_mods_graphql,
            nexus::nexus_fetch_sotf_game_info,
            nexus::nexus_fetch_mod_details,
            nexus::nexus_fetch_mod_files,
            nexus::nexus_fetch_file_dependencies,
            nexus::nexus_fetch_mod_changelogs,
            nexus::nexus_fetch_user_endorsements,
            nexus::nexus_fetch_user_tracked_mods,
            nexus::nexus_endorse_sotf_mod,
            nexus::nexus_track_sotf_mod,
            nexus::nexus_untrack_sotf_mod,
            system::sha256_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
