use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU8, Ordering};
use std::sync::Arc;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlState {
    Running = 0,
    Paused = 1,
    Cancelled = 2,
}

impl ControlState {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => ControlState::Paused,
            2 => ControlState::Cancelled,
            _ => ControlState::Running,
        }
    }
}

/// Shared, thread-safe knobs a running download watches so pause/resume/cancel
/// take effect without tearing down and recreating the async task.
pub struct DownloadControl {
    state: AtomicU8,
    /// Bytes/sec cap; 0 means unlimited. Shared across all active downloads so a
    /// global bandwidth limit can be enforced from Settings.
    pub bandwidth_limit_bps: Arc<AtomicI64>,
}

impl DownloadControl {
    pub fn new(bandwidth_limit_bps: Arc<AtomicI64>) -> Self {
        Self {
            state: AtomicU8::new(ControlState::Running as u8),
            bandwidth_limit_bps,
        }
    }

    pub fn state(&self) -> ControlState {
        ControlState::from_u8(self.state.load(Ordering::SeqCst))
    }

    pub fn pause(&self) {
        self.state.store(ControlState::Paused as u8, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.state.store(ControlState::Running as u8, Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.state.store(ControlState::Cancelled as u8, Ordering::SeqCst);
    }
}

/// A callback invoked periodically with (bytes_downloaded_so_far, total_bytes).
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

pub struct DownloadRequest {
    pub uri: String,
    pub dest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadOutcome {
    Completed,
    Paused,
    Cancelled,
}

/// A download source implementation. Only ordinary HTTP/HTTPS is provided today
/// (`HttpDownloadProvider`); additional lawful providers (e.g. a specific public
/// archive's API) can be added later by implementing this trait and registering
/// with `DownloadManager` — no changes required elsewhere.
#[async_trait::async_trait]
pub trait DownloadProvider: Send + Sync {
    /// Whether this provider knows how to handle the given URI scheme.
    fn supports(&self, uri: &str) -> bool;

    /// Downloads (or resumes downloading) `request.uri` into `request.dest_path`,
    /// reporting progress via `on_progress` and cooperatively honoring
    /// pause/resume/cancel via `control`. Supports resuming a partially written
    /// file via HTTP Range requests when the server allows it.
    async fn download(
        &self,
        request: DownloadRequest,
        control: Arc<DownloadControl>,
        on_progress: ProgressCallback,
    ) -> Result<DownloadOutcome, String>;
}
