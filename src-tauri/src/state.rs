use crate::downloads::DownloadManager;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub http_client: reqwest::Client,
    pub download_manager: DownloadManager,
    pub app_handle: tauri::AppHandle,
    pub covers_dir: PathBuf,
    pub default_install_dir: PathBuf,
    pub default_download_dir: PathBuf,
}
