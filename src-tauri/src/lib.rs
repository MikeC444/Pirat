pub mod commands;
pub mod db;
pub mod downloads;
pub mod events;
pub mod images;
pub mod install;
pub mod logging;
pub mod models;
pub mod settings;
pub mod sources;
pub mod state;

use state::AppState;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let app_handle = app.handle().clone();
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data directory");
            std::fs::create_dir_all(&app_data_dir).ok();

            let _log_guard = logging::init(&app_data_dir.join("logs"));
            // Leak the guard: it must live for the process lifetime to keep the
            // async log writer flushing, and setup() only runs once.
            Box::leak(Box::new(_log_guard));

            let db_path = app_data_dir.join("pirat.sqlite");
            let conn = db::open(&db_path).expect("failed to open database");
            let db = Arc::new(Mutex::new(conn));

            let default_install_dir = app_data_dir.join("Games");
            let default_download_dir = dirs::download_dir()
                .unwrap_or_else(|| app_data_dir.join("Downloads"))
                .join("PiratLauncher");
            let covers_dir = app_data_dir.join("covers");
            std::fs::create_dir_all(&default_install_dir).ok();
            std::fs::create_dir_all(&default_download_dir).ok();
            std::fs::create_dir_all(&covers_dir).ok();

            let http_client = reqwest::Client::builder()
                .build()
                .expect("failed to build HTTP client");

            let max_concurrent = {
                let guard = db.lock().unwrap();
                settings::load(
                    &guard,
                    &default_install_dir.to_string_lossy(),
                    &default_download_dir.to_string_lossy(),
                )
                .max_concurrent_downloads
                .max(1) as usize
            };

            let download_manager = downloads::DownloadManager::new(
                db.clone(),
                app_handle.clone(),
                http_client.clone(),
                max_concurrent,
            );

            {
                let guard = db.lock().unwrap();
                let loaded = settings::load(
                    &guard,
                    &default_install_dir.to_string_lossy(),
                    &default_download_dir.to_string_lossy(),
                );
                download_manager.set_bandwidth_limit_kbps(loaded.bandwidth_limit_kbps);
            }

            app.manage(AppState {
                db: db.clone(),
                http_client: http_client.clone(),
                download_manager,
                app_handle: app_handle.clone(),
                covers_dir,
                default_install_dir,
                default_download_dir,
            });

            // Background scheduler: periodically re-syncs any enabled source whose
            // update interval has elapsed, so the catalogue stays fresh without
            // the user having to manually refresh.
            tauri::async_runtime::spawn(sources::run_scheduler(db, http_client));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_sources,
            commands::add_source,
            commands::set_source_enabled,
            commands::delete_source,
            commands::sync_source,
            commands::sync_all_sources,
            commands::list_games,
            commands::get_game,
            commands::list_genres,
            commands::recently_added,
            commands::list_downloads,
            commands::start_download,
            commands::pause_download,
            commands::resume_download,
            commands::cancel_download,
            commands::install_from_download,
            commands::list_installations,
            commands::get_installation,
            commands::set_install_executable,
            commands::launch_game,
            commands::get_settings,
            commands::save_settings,
            commands::reset_settings,
            commands::clear_image_cache,
            commands::fetch_cover_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
