pub mod catalogue_parser;

use crate::db::{games_repo, sources_repo};
use crate::models::GameSource;
use chrono::Utc;
use rusqlite::Connection;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug)]
pub struct SyncSummary {
    pub games_upserted: usize,
    pub warnings: Vec<String>,
}

/// Fetches the raw catalogue payload for a source. Supports http(s):// via reqwest
/// and file:// (or a bare filesystem path) so the bundled example catalogue and
/// local test fixtures work without a network round trip.
pub async fn fetch_raw(client: &reqwest::Client, url: &str) -> Result<String, String> {
    if let Some(path) = url.strip_prefix("file://") {
        return tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("failed to read local catalogue file {path}: {e}"));
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        // Treat as a bare filesystem path.
        return tokio::fs::read_to_string(url)
            .await
            .map_err(|e| format!("failed to read local catalogue file {url}: {e}"));
    }

    let response = client
        .get(url)
        .header("User-Agent", "PiratLauncher/0.1")
        .send()
        .await
        .map_err(|e| format!("network request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("server responded with HTTP {}", response.status()));
    }

    response
        .text()
        .await
        .map_err(|e| format!("failed to read response body: {e}"))
}

/// Fetches, parses and stores one source's catalogue. On failure, records the
/// error on the source row but leaves any previously cached games untouched so
/// the library keeps working offline.
pub async fn sync_source(
    conn: &Mutex<Connection>,
    client: &reqwest::Client,
    source: &GameSource,
) -> Result<SyncSummary, String> {
    let fetch_result = fetch_raw(client, &source.url).await;
    let now = Utc::now().to_rfc3339();

    let raw = match fetch_result {
        Ok(raw) => raw,
        Err(e) => {
            let guard = conn.lock().map_err(|_| "database lock poisoned".to_string())?;
            let _ = sources_repo::mark_error(&guard, source.id, &e);
            return Err(e);
        }
    };

    let parsed = match catalogue_parser::parse_catalogue(&raw) {
        Ok(p) => p,
        Err(e) => {
            let guard = conn.lock().map_err(|_| "database lock poisoned".to_string())?;
            let _ = sources_repo::mark_error(&guard, source.id, &e);
            return Err(e);
        }
    };

    let guard = conn.lock().map_err(|_| "database lock poisoned".to_string())?;
    for game in &parsed.games {
        games_repo::upsert_from_catalogue(&guard, source.id, game, &now)
            .map_err(|e| format!("database error while storing game {}: {e}", game.external_id))?;
    }
    sources_repo::mark_synced(&guard, source.id, &now)
        .map_err(|e| format!("database error updating source: {e}"))?;

    Ok(SyncSummary {
        games_upserted: parsed.games.len(),
        warnings: parsed.errors,
    })
}

/// Syncs every enabled source once, ignoring individual failures (they're
/// already recorded on the source row) so one broken source doesn't block
/// the others.
pub async fn sync_all_enabled(conn: &Mutex<Connection>, client: &reqwest::Client) {
    let sources = {
        let guard = match conn.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        sources_repo::list(&guard).unwrap_or_default()
    };

    for source in sources.into_iter().filter(|s| s.enabled) {
        let _ = sync_source(conn, client, &source).await;
    }
}

/// Background task: wakes up periodically and syncs any enabled source whose
/// update_interval_minutes has elapsed since last_updated_at.
pub async fn run_scheduler(conn: std::sync::Arc<Mutex<Connection>>, client: reqwest::Client) {
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop {
        tick.tick().await;
        let due = {
            let guard = match conn.lock() {
                Ok(g) => g,
                Err(_) => continue,
            };
            let sources = sources_repo::list(&guard).unwrap_or_default();
            let now = Utc::now();
            sources
                .into_iter()
                .filter(|s| s.enabled)
                .filter(|s| match &s.last_updated_at {
                    None => true,
                    Some(ts) => match chrono::DateTime::parse_from_rfc3339(ts) {
                        Ok(last) => {
                            let elapsed = now.signed_duration_since(last).num_minutes();
                            elapsed >= s.update_interval_minutes
                        }
                        Err(_) => true,
                    },
                })
                .collect::<Vec<_>>()
        };
        for source in due {
            let _ = sync_source(&conn, &client, &source).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::NewGameSource;

    #[tokio::test]
    async fn syncs_local_file_source_and_records_error_when_unavailable() {
        let dir = tempdir();
        let catalogue_path = dir.join("games.json");
        std::fs::write(
            &catalogue_path,
            r#"{"games": [{"id": "g1", "title": "Test Game"}]}"#,
        )
        .unwrap();

        let conn = Mutex::new(db::open_in_memory().unwrap());
        let client = reqwest::Client::new();

        let source_id = {
            let guard = conn.lock().unwrap();
            sources_repo::create(
                &guard,
                &NewGameSource {
                    name: "Local".into(),
                    url: format!("file://{}", catalogue_path.display()),
                    update_interval_minutes: 60,
                },
            )
            .unwrap()
        };
        let source = { sources_repo::get(&conn.lock().unwrap(), source_id).unwrap().unwrap() };

        let summary = sync_source(&conn, &client, &source).await.unwrap();
        assert_eq!(summary.games_upserted, 1);
        assert!(summary.warnings.is_empty());

        let games = { games_repo::query(&conn.lock().unwrap(), &Default::default()).unwrap() };
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Test Game");

        // Now point the source at a missing file: sync should fail but leave the
        // cached game in place, and the error should land on the source row.
        let missing_source = GameSource {
            url: format!("file://{}/missing.json", dir.display()),
            ..source
        };
        let err = sync_source(&conn, &client, &missing_source).await.unwrap_err();
        assert!(err.contains("failed to read local catalogue file"));

        let games_after = { games_repo::query(&conn.lock().unwrap(), &Default::default()).unwrap() };
        assert_eq!(games_after.len(), 1, "cached catalogue should remain available");
    }

    fn tempdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pirat-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
