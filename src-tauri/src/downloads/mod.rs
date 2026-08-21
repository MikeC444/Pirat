pub mod http_provider;
pub mod provider;

use crate::db::downloads_repo;
use crate::events::{DownloadProgressPayload, DOWNLOAD_PROGRESS_EVENT};
use crate::models::DownloadState;
use chrono::Utc;
use provider::{ControlState, DownloadControl, DownloadOutcome, DownloadProvider, DownloadRequest};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::Emitter;
use tokio::sync::Semaphore;

struct ActiveDownload {
    control: Arc<DownloadControl>,
    game_id: i64,
}

/// Coordinates all active downloads: enforces a concurrency limit, applies a
/// shared bandwidth cap, persists progress/state to SQLite, and pushes live
/// progress events to the frontend. Delegates the actual byte transfer to a
/// `DownloadProvider` chosen by URI scheme, so new lawful sources can be added
/// without touching this file.
pub struct DownloadManager {
    conn: Arc<Mutex<Connection>>,
    app_handle: tauri::AppHandle,
    providers: Vec<Arc<dyn DownloadProvider>>,
    active: Arc<Mutex<HashMap<i64, ActiveDownload>>>,
    semaphore: Arc<Semaphore>,
    bandwidth_limit_bps: Arc<AtomicI64>,
}

impl DownloadManager {
    pub fn new(
        conn: Arc<Mutex<Connection>>,
        app_handle: tauri::AppHandle,
        http_client: reqwest::Client,
        max_concurrent: usize,
    ) -> Self {
        let http_provider: Arc<dyn DownloadProvider> = Arc::new(http_provider::HttpDownloadProvider::new(http_client));
        Self {
            conn,
            app_handle,
            providers: vec![http_provider],
            active: Arc::new(Mutex::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
            bandwidth_limit_bps: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn set_bandwidth_limit_kbps(&self, kbps: i64) {
        let bps = if kbps <= 0 { 0 } else { kbps * 1024 };
        self.bandwidth_limit_bps.store(bps, Ordering::Relaxed);
    }

    pub fn set_max_concurrent(&self, _max: usize) {
        // tokio::sync::Semaphore has no safe shrink; permits are re-created on next
        // manager construction (app restart / settings apply). Acceptable for a
        // desktop app where this changes rarely.
    }

    fn pick_provider(&self, uri: &str) -> Option<Arc<dyn DownloadProvider>> {
        self.providers.iter().find(|p| p.supports(uri)).cloned()
    }

    /// Queues a new download for `game_id` from `download_url` into `dest_path`,
    /// returning the created download row id. The actual transfer starts as soon
    /// as a concurrency slot is free.
    pub fn start_download(&self, game_id: i64, download_url: String, dest_path: PathBuf) -> Result<i64, String> {
        if self.pick_provider(&download_url).is_none() {
            return Err(format!("no download provider supports this URI: {download_url}"));
        }

        let now = Utc::now().to_rfc3339();
        let download_id = {
            let conn = self.conn.lock().map_err(|_| "database lock poisoned")?;
            downloads_repo::create(&conn, game_id, dest_path.to_string_lossy().as_ref(), &now)
                .map_err(|e| format!("failed to create download record: {e}"))?
        };

        self.spawn_transfer(download_id, game_id, download_url, dest_path);
        Ok(download_id)
    }

    /// Resumes a paused/failed download. If the transfer task is still alive
    /// (paused in place), simply flips its control state; otherwise starts a
    /// fresh transfer, which itself resumes from the partially written file via
    /// an HTTP Range request.
    pub fn resume_download_by_id(&self, download_id: i64, game_id: i64, download_url: String, dest_path: PathBuf) {
        let still_active = {
            let active = self.active.lock().unwrap();
            if let Some(a) = active.get(&download_id) {
                a.control.resume();
                true
            } else {
                false
            }
        };
        if still_active {
            let now = Utc::now().to_rfc3339();
            if let Ok(c) = self.conn.lock() {
                let _ = downloads_repo::set_state(&c, download_id, DownloadState::Downloading.as_str(), &now);
            }
            self.emit_state(download_id, game_id, DownloadState::Downloading, None);
            return;
        }
        self.spawn_transfer(download_id, game_id, download_url, dest_path);
    }

    fn emit_state(&self, download_id: i64, game_id: i64, state: DownloadState, error: Option<String>) {
        let (progress_bytes, total_bytes) = {
            if let Ok(c) = self.conn.lock() {
                downloads_repo::get(&c, download_id)
                    .ok()
                    .flatten()
                    .map(|d| (d.progress_bytes, d.total_bytes))
                    .unwrap_or((0, 0))
            } else {
                (0, 0)
            }
        };
        let _ = self.app_handle.emit(
            DOWNLOAD_PROGRESS_EVENT,
            DownloadProgressPayload {
                download_id,
                game_id,
                state: state.as_str().to_string(),
                progress_bytes,
                total_bytes,
                speed_bps: 0.0,
                eta_seconds: None,
                error,
            },
        );
    }

    fn spawn_transfer(&self, download_id: i64, game_id: i64, download_url: String, dest_path: PathBuf) {
        let provider = match self.pick_provider(&download_url) {
            Some(p) => p,
            None => return,
        };
        let control = Arc::new(DownloadControl::new(self.bandwidth_limit_bps.clone()));
        self.active.lock().unwrap().insert(
            download_id,
            ActiveDownload {
                control: control.clone(),
                game_id,
            },
        );

        let conn = self.conn.clone();
        let app_handle = self.app_handle.clone();
        let semaphore = self.semaphore.clone();
        let active = self.active.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await;
            {
                let c = conn.lock().unwrap();
                let now = Utc::now().to_rfc3339();
                let _ = downloads_repo::set_started(&c, download_id, &now);
                let _ = downloads_repo::set_state(&c, download_id, DownloadState::Downloading.as_str(), &now);
            }

            let conn_for_progress = conn.clone();
            let app_for_progress = app_handle.clone();
            let last_emit = Arc::new(Mutex::new(Instant::now()));
            let last_bytes = Arc::new(Mutex::new(0u64));
            let last_time = Arc::new(Mutex::new(Instant::now()));

            let on_progress: provider::ProgressCallback = Box::new(move |downloaded, total| {
                let mut le = last_emit.lock().unwrap();
                if le.elapsed().as_millis() < 250 && downloaded < total {
                    return;
                }
                *le = Instant::now();

                let mut lb = last_bytes.lock().unwrap();
                let mut lt = last_time.lock().unwrap();
                let elapsed = lt.elapsed().as_secs_f64().max(0.001);
                let speed = ((downloaded.saturating_sub(*lb)) as f64) / elapsed;
                *lb = downloaded;
                *lt = Instant::now();

                let now = Utc::now().to_rfc3339();
                if let Ok(c) = conn_for_progress.lock() {
                    let _ = downloads_repo::update_progress(&c, download_id, downloaded as i64, total as i64, speed, &now);
                }
                let eta = if speed > 1.0 && total > downloaded {
                    Some((total - downloaded) as f64 / speed)
                } else {
                    None
                };
                let _ = app_for_progress.emit(
                    DOWNLOAD_PROGRESS_EVENT,
                    DownloadProgressPayload {
                        download_id,
                        game_id,
                        state: DownloadState::Downloading.as_str().to_string(),
                        progress_bytes: downloaded as i64,
                        total_bytes: total as i64,
                        speed_bps: speed,
                        eta_seconds: eta,
                        error: None,
                    },
                );
            });

            let result = provider
                .download(
                    DownloadRequest {
                        uri: download_url,
                        dest_path,
                    },
                    control,
                    on_progress,
                )
                .await;

            active.lock().unwrap().remove(&download_id);

            let now = Utc::now().to_rfc3339();
            let final_state = match &result {
                Ok(DownloadOutcome::Completed) => {
                    let c = conn.lock().unwrap();
                    let _ = downloads_repo::set_state(&c, download_id, DownloadState::Completed.as_str(), &now);
                    DownloadState::Completed
                }
                Ok(DownloadOutcome::Paused) => {
                    let c = conn.lock().unwrap();
                    let _ = downloads_repo::set_state(&c, download_id, DownloadState::Paused.as_str(), &now);
                    DownloadState::Paused
                }
                Ok(DownloadOutcome::Cancelled) => {
                    let c = conn.lock().unwrap();
                    let _ = downloads_repo::set_state(&c, download_id, DownloadState::Cancelled.as_str(), &now);
                    DownloadState::Cancelled
                }
                Err(e) => {
                    let c = conn.lock().unwrap();
                    let _ = downloads_repo::set_error(&c, download_id, e, &now);
                    DownloadState::Failed
                }
            };

            let (progress_bytes, total_bytes) = {
                let c = conn.lock().unwrap();
                downloads_repo::get(&c, download_id)
                    .ok()
                    .flatten()
                    .map(|d| (d.progress_bytes, d.total_bytes))
                    .unwrap_or((0, 0))
            };

            let _ = app_handle.emit(
                DOWNLOAD_PROGRESS_EVENT,
                DownloadProgressPayload {
                    download_id,
                    game_id,
                    state: final_state.as_str().to_string(),
                    progress_bytes,
                    total_bytes,
                    speed_bps: 0.0,
                    eta_seconds: None,
                    error: result.err(),
                },
            );
        });
    }

    pub fn pause(&self, download_id: i64) -> Result<(), String> {
        let game_id = {
            let active = self.active.lock().unwrap();
            match active.get(&download_id) {
                Some(a) => {
                    a.control.pause();
                    a.game_id
                }
                None => return Err("download is not currently active".to_string()),
            }
        };
        let now = Utc::now().to_rfc3339();
        if let Ok(c) = self.conn.lock() {
            let _ = downloads_repo::set_state(&c, download_id, DownloadState::Paused.as_str(), &now);
        }
        self.emit_state(download_id, game_id, DownloadState::Paused, None);
        Ok(())
    }

    pub fn cancel(&self, download_id: i64) -> Result<(), String> {
        let active = self.active.lock().unwrap();
        match active.get(&download_id) {
            Some(a) => {
                a.control.cancel();
                Ok(())
            }
            None => {
                // Not running (e.g. already failed/paused): just mark cancelled and
                // best-effort delete the partial file.
                drop(active);
                let now = Utc::now().to_rfc3339();
                let conn = self.conn.lock().map_err(|_| "database lock poisoned")?;
                if let Ok(Some(task)) = downloads_repo::get(&conn, download_id) {
                    let _ = std::fs::remove_file(&task.dest_path);
                }
                downloads_repo::set_state(&conn, download_id, DownloadState::Cancelled.as_str(), &now)
                    .map_err(|e| e.to_string())
            }
        }
    }

    pub fn control_state(&self, download_id: i64) -> Option<ControlState> {
        self.active.lock().unwrap().get(&download_id).map(|a| a.control.state())
    }
}
