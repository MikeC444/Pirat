use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Downloads and disk-caches cover art so the same URL is never fetched twice.
/// Cache key is a hash of the URL (not the title/id), so it stays correct even
/// if a catalogue reuses art across entries or changes unrelated fields.
pub async fn get_or_fetch(client: &reqwest::Client, cache_dir: &Path, url: &str) -> Result<PathBuf, String> {
    tokio::fs::create_dir_all(cache_dir)
        .await
        .map_err(|e| format!("failed to create image cache directory: {e}"))?;

    let ext = guess_extension(url);
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let path = cache_dir.join(format!("{hash}.{ext}"));

    if tokio::fs::metadata(&path).await.is_ok() {
        return Ok(path);
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("refusing to fetch cover art from non-http(s) URL: {url}"));
    }

    let response = client
        .get(url)
        .header("User-Agent", "PiratLauncher/0.1")
        .send()
        .await
        .map_err(|e| format!("failed to fetch cover art: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("cover art server responded with HTTP {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|e| format!("failed to read cover art response: {e}"))?;

    let tmp_path = path.with_extension(format!("{ext}.part"));
    tokio::fs::write(&tmp_path, &bytes)
        .await
        .map_err(|e| format!("failed to write cover art to cache: {e}"))?;
    tokio::fs::rename(&tmp_path, &path)
        .await
        .map_err(|e| format!("failed to finalize cached cover art: {e}"))?;

    Ok(path)
}

fn guess_extension(url: &str) -> &'static str {
    let lower = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    if lower.ends_with(".png") {
        "png"
    } else if lower.ends_with(".webp") {
        "webp"
    } else if lower.ends_with(".gif") {
        "gif"
    } else {
        "jpg"
    }
}

/// Deletes every cached cover art file. Used by Settings → Clear Cache.
pub fn clear_cache(cache_dir: &Path) -> Result<(), String> {
    if !cache_dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(cache_dir).map_err(|e| format!("failed to clear image cache: {e}"))?;
    std::fs::create_dir_all(cache_dir).map_err(|e| format!("failed to recreate image cache directory: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_guessing() {
        assert_eq!(guess_extension("https://x.com/a.PNG?x=1"), "png");
        assert_eq!(guess_extension("https://x.com/a.webp"), "webp");
        assert_eq!(guess_extension("https://x.com/a"), "jpg");
    }

    #[tokio::test]
    async fn rejects_non_http_urls() {
        let dir = std::env::temp_dir().join(format!("pirat-imgcache-{}", uuid::Uuid::new_v4()));
        let client = reqwest::Client::new();
        let err = get_or_fetch(&client, &dir, "file:///etc/passwd").await.unwrap_err();
        assert!(err.contains("non-http"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
