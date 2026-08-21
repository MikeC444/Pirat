use crate::db::{downloads_repo, games_repo, installs_repo, sources_repo};
use crate::events::{SourceSyncedPayload, SOURCE_SYNCED_EVENT};
use crate::models::{AppSettings, DownloadTask, Game, GameSource, Installation, NewGameSource};
use crate::state::AppState;
use crate::{images, install, settings, sources};
use serde::Serialize;
use tauri::{Emitter, State};

fn lock_err(_: impl std::fmt::Debug) -> String {
    "database lock poisoned".to_string()
}

// ---------- Sources ----------

#[tauri::command]
pub fn list_sources(state: State<AppState>) -> Result<Vec<GameSource>, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    sources_repo::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_source(
    state: State<'_, AppState>,
    name: String,
    url: String,
    update_interval_minutes: i64,
) -> Result<GameSource, String> {
    if url.trim().is_empty() {
        return Err("source URL cannot be empty".to_string());
    }
    if !url.starts_with("https://") && !url.starts_with("http://") && !url.starts_with("file://") {
        return Err("source URL must be http://, https://, or file://".to_string());
    }
    let id = {
        let conn = state.db.lock().map_err(lock_err)?;
        sources_repo::create(
            &conn,
            &NewGameSource {
                name,
                url,
                update_interval_minutes: update_interval_minutes.max(5),
            },
        )
        .map_err(|e| e.to_string())?
    };
    let source = {
        let conn = state.db.lock().map_err(lock_err)?;
        sources_repo::get(&conn, id).map_err(|e| e.to_string())?.ok_or("source vanished after insert")?
    };

    // Kick off an immediate sync so the new source's games show up right away.
    let _ = sync_source_inner(&state, &source).await;

    let conn = state.db.lock().map_err(lock_err)?;
    sources_repo::get(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "source not found".to_string())
}

#[tauri::command]
pub fn set_source_enabled(state: State<AppState>, id: i64, enabled: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(lock_err)?;
    sources_repo::set_enabled(&conn, id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_source(state: State<AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(lock_err)?;
    sources_repo::delete(&conn, id).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct SyncOutcome {
    pub games_upserted: usize,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

async fn sync_source_inner(state: &State<'_, AppState>, source: &GameSource) -> SyncOutcome {
    let result = sources::sync_source(&state.db, &state.http_client, source).await;
    let outcome = match &result {
        Ok(summary) => SyncOutcome {
            games_upserted: summary.games_upserted,
            warnings: summary.warnings.clone(),
            error: None,
        },
        Err(e) => SyncOutcome {
            games_upserted: 0,
            warnings: vec![],
            error: Some(e.clone()),
        },
    };
    let _ = state.app_handle.emit(
        SOURCE_SYNCED_EVENT,
        SourceSyncedPayload {
            source_id: source.id,
            games_upserted: outcome.games_upserted,
            warnings: outcome.warnings.clone(),
            error: outcome.error.clone(),
        },
    );
    outcome
}

#[tauri::command]
pub async fn sync_source(state: State<'_, AppState>, id: i64) -> Result<SyncOutcome, String> {
    let source = {
        let conn = state.db.lock().map_err(lock_err)?;
        sources_repo::get(&conn, id).map_err(|e| e.to_string())?.ok_or("source not found")?
    };
    Ok(sync_source_inner(&state, &source).await)
}

#[tauri::command]
pub async fn sync_all_sources(state: State<'_, AppState>) -> Result<(), String> {
    sources::sync_all_enabled(&state.db, &state.http_client).await;
    Ok(())
}

// ---------- Games ----------

#[tauri::command]
pub fn list_games(state: State<AppState>, query: games_repo::GameQuery) -> Result<Vec<Game>, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    games_repo::query(&conn, &query).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_game(state: State<AppState>, id: i64) -> Result<Option<Game>, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    games_repo::get(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_genres(state: State<AppState>) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    games_repo::distinct_genres(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recently_added(state: State<AppState>, limit: i64) -> Result<Vec<Game>, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    games_repo::recently_added(&conn, limit).map_err(|e| e.to_string())
}

// ---------- Downloads ----------

#[tauri::command]
pub fn list_downloads(state: State<AppState>) -> Result<Vec<DownloadTask>, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    downloads_repo::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_download(state: State<AppState>, game_id: i64) -> Result<i64, String> {
    let game = {
        let conn = state.db.lock().map_err(lock_err)?;
        games_repo::get(&conn, game_id).map_err(|e| e.to_string())?.ok_or("game not found")?
    };
    let download_url = game.download_url.clone().ok_or("this game has no download source configured")?;
    let dest_dir = state.default_download_dir.clone();
    let file_name = download_url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("download.bin")
        .to_string();
    let dest_path = dest_dir.join(format!("{game_id}-{file_name}"));
    state.download_manager.start_download(game_id, download_url, dest_path)
}

#[tauri::command]
pub fn pause_download(state: State<AppState>, download_id: i64) -> Result<(), String> {
    state.download_manager.pause(download_id)
}

#[tauri::command]
pub fn resume_download(state: State<AppState>, download_id: i64) -> Result<(), String> {
    let (game_id, dest_path, download_url) = {
        let conn = state.db.lock().map_err(lock_err)?;
        let task = downloads_repo::get(&conn, download_id).map_err(|e| e.to_string())?.ok_or("download not found")?;
        let game = games_repo::get(&conn, task.game_id).map_err(|e| e.to_string())?.ok_or("game not found")?;
        let url = game.download_url.ok_or("this game has no download source configured")?;
        (task.game_id, std::path::PathBuf::from(task.dest_path), url)
    };
    state.download_manager.resume_download_by_id(download_id, game_id, download_url, dest_path);
    Ok(())
}

#[tauri::command]
pub fn cancel_download(state: State<AppState>, download_id: i64) -> Result<(), String> {
    state.download_manager.cancel(download_id)
}

// ---------- Install ----------

#[derive(Serialize)]
pub struct InstallOutcome {
    pub install_path: String,
    pub suggested_executable: Option<String>,
}

#[tauri::command]
pub fn install_from_download(state: State<AppState>, download_id: i64) -> Result<InstallOutcome, String> {
    let (task, game) = {
        let conn = state.db.lock().map_err(lock_err)?;
        let task = downloads_repo::get(&conn, download_id).map_err(|e| e.to_string())?.ok_or("download not found")?;
        let game = games_repo::get(&conn, task.game_id).map_err(|e| e.to_string())?.ok_or("game not found")?;
        (task, game)
    };
    if task.state != "completed" {
        return Err("download has not completed yet".to_string());
    }

    let result = install::install_downloaded_file(
        std::path::Path::new(&task.dest_path),
        &state.default_install_dir,
        &game.title,
        game.checksum.as_deref(),
        game.checksum_algo.as_deref(),
    )?;

    let now = chrono::Utc::now().to_rfc3339();
    {
        let conn = state.db.lock().map_err(lock_err)?;
        installs_repo::upsert(
            &conn,
            game.id,
            &result.install_path.to_string_lossy(),
            result.suggested_executable.as_ref().map(|p| p.to_string_lossy()).as_deref(),
            game.version.as_deref(),
            &now,
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(InstallOutcome {
        install_path: result.install_path.to_string_lossy().to_string(),
        suggested_executable: result.suggested_executable.map(|p| p.to_string_lossy().to_string()),
    })
}

#[tauri::command]
pub fn list_installations(state: State<AppState>) -> Result<Vec<Installation>, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    installs_repo::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_installation(state: State<AppState>, game_id: i64) -> Result<Option<Installation>, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    installs_repo::get_by_game(&conn, game_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_install_executable(state: State<AppState>, game_id: i64, executable_path: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(lock_err)?;
    installs_repo::set_executable(&conn, game_id, &executable_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn launch_game(state: State<AppState>, game_id: i64) -> Result<(), String> {
    let install = {
        let conn = state.db.lock().map_err(lock_err)?;
        installs_repo::get_by_game(&conn, game_id).map_err(|e| e.to_string())?.ok_or("game is not installed")?
    };
    let exe = install.executable_path.ok_or("no launch executable configured for this game")?;
    install::launch_executable(std::path::Path::new(&exe))
}

// ---------- Settings ----------

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(settings::load(
        &conn,
        &state.default_install_dir.to_string_lossy(),
        &state.default_download_dir.to_string_lossy(),
    ))
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, new_settings: AppSettings) -> Result<(), String> {
    let conn = state.db.lock().map_err(lock_err)?;
    settings::save(&conn, &new_settings)?;
    state.download_manager.set_bandwidth_limit_kbps(new_settings.bandwidth_limit_kbps);
    state.download_manager.set_max_concurrent(new_settings.max_concurrent_downloads.max(1) as usize);
    Ok(())
}

#[tauri::command]
pub fn reset_settings(state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(lock_err)?;
    settings::reset(&conn)
}

#[tauri::command]
pub fn clear_image_cache(state: State<AppState>) -> Result<(), String> {
    images::clear_cache(&state.covers_dir)
}

// ---------- Images ----------

#[tauri::command]
pub async fn fetch_cover_image(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let path = images::get_or_fetch(&state.http_client, &state.covers_dir, &url).await?;
    Ok(path.to_string_lossy().to_string())
}
