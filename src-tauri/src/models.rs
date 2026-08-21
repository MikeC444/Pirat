use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSource {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub update_interval_minutes: i64,
    pub last_updated_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewGameSource {
    pub name: String,
    pub url: String,
    pub update_interval_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Game {
    pub id: i64,
    pub source_id: i64,
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub cover_url: Option<String>,
    pub version: Option<String>,
    pub size_bytes: Option<i64>,
    pub release_date: Option<String>,
    pub genres: Vec<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub download_url: Option<String>,
    pub checksum: Option<String>,
    pub checksum_algo: Option<String>,
    pub added_at: String,
    pub updated_at: String,
}

/// Normalized game data straight out of a catalogue JSON payload, before it is
/// assigned a database row id / source id.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogueGame {
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub cover_url: Option<String>,
    pub version: Option<String>,
    pub size_bytes: Option<i64>,
    pub release_date: Option<String>,
    pub genres: Vec<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub download_url: Option<String>,
    pub checksum: Option<String>,
    pub checksum_algo: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadState::Queued => "queued",
            DownloadState::Downloading => "downloading",
            DownloadState::Paused => "paused",
            DownloadState::Completed => "completed",
            DownloadState::Failed => "failed",
            DownloadState::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "downloading" => DownloadState::Downloading,
            "paused" => DownloadState::Paused,
            "completed" => DownloadState::Completed,
            "failed" => DownloadState::Failed,
            "cancelled" => DownloadState::Cancelled,
            _ => DownloadState::Queued,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: i64,
    pub game_id: i64,
    pub state: String,
    pub progress_bytes: i64,
    pub total_bytes: i64,
    pub speed_bps: f64,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub error: Option<String>,
    pub dest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installation {
    pub id: i64,
    pub game_id: i64,
    pub install_path: String,
    pub executable_path: Option<String>,
    pub installed_at: String,
    pub version_installed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub install_dir: String,
    pub download_dir: String,
    pub max_concurrent_downloads: i64,
    pub bandwidth_limit_kbps: i64,
    pub theme: String,
    pub start_with_windows: bool,
    pub auto_update_sources: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            install_dir: String::new(),
            download_dir: String::new(),
            max_concurrent_downloads: 2,
            bandwidth_limit_kbps: 0,
            theme: "dark".to_string(),
            start_with_windows: false,
            auto_update_sources: true,
        }
    }
}
