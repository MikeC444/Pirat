use serde::Serialize;

pub const DOWNLOAD_PROGRESS_EVENT: &str = "download://progress";
pub const SOURCE_SYNCED_EVENT: &str = "source://synced";

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgressPayload {
    pub download_id: i64,
    pub game_id: i64,
    pub state: String,
    pub progress_bytes: i64,
    pub total_bytes: i64,
    pub speed_bps: f64,
    pub eta_seconds: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSyncedPayload {
    pub source_id: i64,
    pub games_upserted: usize,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}
