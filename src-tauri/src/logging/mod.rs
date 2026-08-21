use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Initializes application logging: human-readable console output plus a
/// plain log file under the app's data directory, filtered by `RUST_LOG`
/// (defaulting to info-level) so a user can turn on `debug`/`trace` without a
/// rebuild. Returns a guard that must be kept alive for the process lifetime
/// to ensure buffered log lines are flushed.
pub fn init(log_dir: &Path) -> WorkerGuard {
    let _ = std::fs::create_dir_all(log_dir);
    let file_appender = tracing_appender::rolling::never(log_dir, "pirat-launcher.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .try_init();

    guard
}
