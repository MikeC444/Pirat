use crate::db::settings_repo;
use crate::models::AppSettings;
use rusqlite::Connection;

const KEY_INSTALL_DIR: &str = "install_dir";
const KEY_DOWNLOAD_DIR: &str = "download_dir";
const KEY_MAX_CONCURRENT: &str = "max_concurrent_downloads";
const KEY_BANDWIDTH_LIMIT: &str = "bandwidth_limit_kbps";
const KEY_THEME: &str = "theme";
const KEY_START_WITH_WINDOWS: &str = "start_with_windows";
const KEY_AUTO_UPDATE_SOURCES: &str = "auto_update_sources";

pub fn load(conn: &Connection, default_install_dir: &str, default_download_dir: &str) -> AppSettings {
    let mut settings = AppSettings::default();
    settings.install_dir = default_install_dir.to_string();
    settings.download_dir = default_download_dir.to_string();

    if let Ok(Some(v)) = settings_repo::get_raw(conn, KEY_INSTALL_DIR) {
        settings.install_dir = v;
    }
    if let Ok(Some(v)) = settings_repo::get_raw(conn, KEY_DOWNLOAD_DIR) {
        settings.download_dir = v;
    }
    if let Ok(Some(v)) = settings_repo::get_raw(conn, KEY_MAX_CONCURRENT) {
        if let Ok(n) = v.parse() {
            settings.max_concurrent_downloads = n;
        }
    }
    if let Ok(Some(v)) = settings_repo::get_raw(conn, KEY_BANDWIDTH_LIMIT) {
        if let Ok(n) = v.parse() {
            settings.bandwidth_limit_kbps = n;
        }
    }
    if let Ok(Some(v)) = settings_repo::get_raw(conn, KEY_THEME) {
        settings.theme = v;
    }
    if let Ok(Some(v)) = settings_repo::get_raw(conn, KEY_START_WITH_WINDOWS) {
        settings.start_with_windows = v == "true";
    }
    if let Ok(Some(v)) = settings_repo::get_raw(conn, KEY_AUTO_UPDATE_SOURCES) {
        settings.auto_update_sources = v == "true";
    }

    settings
}

pub fn save(conn: &Connection, settings: &AppSettings) -> Result<(), String> {
    settings_repo::set_raw(conn, KEY_INSTALL_DIR, &settings.install_dir).map_err(|e| e.to_string())?;
    settings_repo::set_raw(conn, KEY_DOWNLOAD_DIR, &settings.download_dir).map_err(|e| e.to_string())?;
    settings_repo::set_raw(conn, KEY_MAX_CONCURRENT, &settings.max_concurrent_downloads.to_string())
        .map_err(|e| e.to_string())?;
    settings_repo::set_raw(conn, KEY_BANDWIDTH_LIMIT, &settings.bandwidth_limit_kbps.to_string())
        .map_err(|e| e.to_string())?;
    settings_repo::set_raw(conn, KEY_THEME, &settings.theme).map_err(|e| e.to_string())?;
    settings_repo::set_raw(
        conn,
        KEY_START_WITH_WINDOWS,
        if settings.start_with_windows { "true" } else { "false" },
    )
    .map_err(|e| e.to_string())?;
    settings_repo::set_raw(
        conn,
        KEY_AUTO_UPDATE_SOURCES,
        if settings.auto_update_sources { "true" } else { "false" },
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn reset(conn: &Connection) -> Result<(), String> {
    settings_repo::clear_all(conn).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn load_defaults_then_save_and_reload() {
        let conn = db::open_in_memory().unwrap();
        let defaults = load(&conn, "/games", "/downloads");
        assert_eq!(defaults.install_dir, "/games");
        assert_eq!(defaults.max_concurrent_downloads, 2);

        let mut updated = defaults.clone();
        updated.max_concurrent_downloads = 5;
        updated.theme = "light".to_string();
        updated.start_with_windows = true;
        save(&conn, &updated).unwrap();

        let reloaded = load(&conn, "/games", "/downloads");
        assert_eq!(reloaded.max_concurrent_downloads, 5);
        assert_eq!(reloaded.theme, "light");
        assert!(reloaded.start_with_windows);

        reset(&conn).unwrap();
        let after_reset = load(&conn, "/games", "/downloads");
        assert_eq!(after_reset.max_concurrent_downloads, 2);
    }
}
