use super::provider::{ControlState, DownloadControl, DownloadOutcome, DownloadProvider, DownloadRequest, ProgressCallback};
use futures_util::StreamExt;
use std::io::SeekFrom;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

/// Ordinary HTTP/HTTPS downloader. Resumes partially-downloaded files using
/// `Range` requests when the server advertises byte-range support, honors
/// cooperative pause/cancel via `DownloadControl`, and applies a shared
/// bandwidth cap by sleeping between chunks.
pub struct HttpDownloadProvider {
    client: reqwest::Client,
}

impl HttpDownloadProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

const CHUNK_TARGET: usize = 64 * 1024;

#[async_trait::async_trait]
impl DownloadProvider for HttpDownloadProvider {
    fn supports(&self, uri: &str) -> bool {
        uri.starts_with("http://") || uri.starts_with("https://")
    }

    async fn download(
        &self,
        request: DownloadRequest,
        control: Arc<DownloadControl>,
        on_progress: ProgressCallback,
    ) -> Result<DownloadOutcome, String> {
        if let Some(parent) = request.dest_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create download directory: {e}"))?;
        }

        let already_downloaded = tokio::fs::metadata(&request.dest_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let mut req_builder = self.client.get(&request.uri).header("User-Agent", "PiratLauncher/0.1");
        if already_downloaded > 0 {
            req_builder = req_builder.header("Range", format!("bytes={already_downloaded}-"));
        }

        let response = req_builder.send().await.map_err(|e| format!("download request failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("server responded with HTTP {}", response.status()));
        }

        let resumed = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let start_offset = if resumed { already_downloaded } else { 0 };
        let content_length = response.content_length().unwrap_or(0);
        let total_bytes = if resumed { start_offset + content_length } else { content_length };

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&request.dest_path)
            .await
            .map_err(|e| format!("failed to open destination file: {e}"))?;

        if resumed {
            file.seek(SeekFrom::Start(start_offset))
                .await
                .map_err(|e| format!("failed to seek destination file: {e}"))?;
        } else {
            file.set_len(0).await.ok();
        }

        let mut downloaded: u64 = start_offset;
        let mut stream = response.bytes_stream();
        let mut pending: Vec<u8> = Vec::with_capacity(CHUNK_TARGET);

        loop {
            match control.state() {
                ControlState::Cancelled => {
                    drop(file);
                    let _ = tokio::fs::remove_file(&request.dest_path).await;
                    return Ok(DownloadOutcome::Cancelled);
                }
                ControlState::Paused => {
                    file.flush().await.ok();
                    return Ok(DownloadOutcome::Paused);
                }
                ControlState::Running => {}
            }

            let next = tokio::time::timeout(Duration::from_secs(30), stream.next()).await;
            let chunk = match next {
                Ok(Some(Ok(bytes))) => bytes,
                Ok(Some(Err(e))) => return Err(format!("download stream error: {e}")),
                Ok(None) => break,
                Err(_) => return Err("download stalled: no data received in 30s".to_string()),
            };

            pending.extend_from_slice(&chunk);
            downloaded += chunk.len() as u64;

            file.write_all(&chunk)
                .await
                .map_err(|e| format!("failed to write to destination file: {e}"))?;

            on_progress(downloaded, total_bytes);

            let limit = control.bandwidth_limit_bps.load(Ordering::Relaxed);
            if limit > 0 && pending.len() >= CHUNK_TARGET {
                let sleep_secs = pending.len() as f64 / limit as f64;
                pending.clear();
                if sleep_secs > 0.0 {
                    tokio::time::sleep(Duration::from_secs_f64(sleep_secs)).await;
                }
            }
        }

        file.flush().await.map_err(|e| format!("failed to flush destination file: {e}"))?;
        Ok(DownloadOutcome::Completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicI64;

    /// Minimal hand-rolled HTTP/1.1 server that always answers a GET with the
    /// given body (ignoring Range) — enough to exercise the provider's happy
    /// path without pulling in a full server framework as a test dependency.
    async fn serve_once(body: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.write_all(&body).await;
            let _ = stream.shutdown().await;
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn downloads_full_file_and_reports_completion() {
        let body = b"hello world, this is a test download payload".to_vec();
        let (url, _server) = serve_once(body.clone()).await;

        let provider = HttpDownloadProvider::new(reqwest::Client::new());
        let dest = std::env::temp_dir().join(format!("pirat-dl-test-{}.bin", uuid::Uuid::new_v4()));
        let control = Arc::new(DownloadControl::new(Arc::new(AtomicI64::new(0))));

        let outcome = provider
            .download(
                DownloadRequest {
                    uri: url,
                    dest_path: dest.clone(),
                },
                control,
                Box::new(|_, _| {}),
            )
            .await
            .unwrap();

        assert_eq!(outcome, DownloadOutcome::Completed);
        let written = tokio::fs::read(&dest).await.unwrap();
        assert_eq!(written, body);
        let _ = tokio::fs::remove_file(&dest).await;
    }

    #[test]
    fn supports_only_http_and_https() {
        let provider = HttpDownloadProvider::new(reqwest::Client::new());
        assert!(provider.supports("https://example.com/a.zip"));
        assert!(provider.supports("http://example.com/a.zip"));
        assert!(!provider.supports("ftp://example.com/a.zip"));
        assert!(!provider.supports("magnet:?xt=..."));
    }
}
