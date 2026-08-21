pub mod archive;

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct InstallResult {
    pub install_path: PathBuf,
    /// Best-effort guess at the game's launchable executable, offered to the
    /// user as a suggestion only. Nothing is ever run automatically.
    pub suggested_executable: Option<PathBuf>,
}

/// Verifies `path`'s checksum against `expected_hex` using `algo`. Only
/// `sha256` is supported today; an unrecognized algorithm is a hard error
/// rather than silently skipping verification, so a catalogue can't quietly
/// disable integrity checking by naming an algorithm we don't implement.
pub fn verify_checksum(path: &Path, expected_hex: &str, algo: &str) -> Result<(), String> {
    if !algo.eq_ignore_ascii_case("sha256") {
        return Err(format!("unsupported checksum algorithm: {algo}"));
    }

    let mut file = fs::File::open(path).map_err(|e| format!("failed to open downloaded file: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| format!("failed to read downloaded file: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());

    if actual.eq_ignore_ascii_case(expected_hex.trim()) {
        Ok(())
    } else {
        Err(format!(
            "checksum mismatch: expected {expected_hex}, got {actual} — the download may be corrupted or tampered with"
        ))
    }
}

/// Produces a filesystem-safe directory name from a game title.
fn sanitize_dir_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "game".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Runs the install pipeline for an already-downloaded file: verifies its
/// checksum when one is available, extracts it if it's a zip archive (using
/// the zip-slip-safe extractor), or otherwise places the file as-is into the
/// install directory. Never executes anything.
pub fn install_downloaded_file(
    downloaded_file: &Path,
    install_root: &Path,
    game_title: &str,
    checksum: Option<&str>,
    checksum_algo: Option<&str>,
) -> Result<InstallResult, String> {
    if let (Some(expected), Some(algo)) = (checksum, checksum_algo) {
        verify_checksum(downloaded_file, expected, algo)?;
    }

    let install_dir = install_root.join(sanitize_dir_name(game_title));
    fs::create_dir_all(&install_dir).map_err(|e| format!("failed to create install directory: {e}"))?;

    let is_zip = downloaded_file
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);

    if is_zip {
        archive::extract_zip_safe(downloaded_file, &install_dir)?;
    } else {
        let file_name = downloaded_file
            .file_name()
            .ok_or_else(|| "downloaded file has no file name".to_string())?;
        let dest = install_dir.join(file_name);
        fs::copy(downloaded_file, &dest).map_err(|e| format!("failed to place downloaded file: {e}"))?;
    }

    let suggested_executable = find_likely_executable(&install_dir);

    Ok(InstallResult {
        install_path: install_dir,
        suggested_executable,
    })
}

/// Scans up to two directory levels for a `.exe` file to suggest as the
/// launch target. Purely advisory — the user (or Settings) must explicitly
/// confirm it before "Launch" will run it.
fn find_likely_executable(dir: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    collect_exes(dir, 0, &mut candidates);
    // Prefer shorter paths (top-level exes over ones buried in subfolders).
    candidates.sort_by_key(|p| p.components().count());
    candidates.into_iter().next()
}

fn collect_exes(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    if depth > 2 {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_exes(&path, depth + 1, out);
        } else if path.extension().map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Launches an installed game's executable. Only ever called on a path taken
/// from an `installations` DB record the user explicitly set up — never on an
/// arbitrary or freshly-downloaded file. Uses an argument array (no shell) to
/// avoid any command-injection risk.
pub fn launch_executable(executable_path: &Path) -> Result<(), String> {
    if !executable_path.is_file() {
        return Err(format!("executable not found: {}", executable_path.display()));
    }
    std::process::Command::new(executable_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to launch {}: {e}", executable_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn checksum_matches_and_mismatches() {
        let path = std::env::temp_dir().join(format!("pirat-checksum-test-{}.bin", uuid::Uuid::new_v4()));
        fs::write(&path, b"the quick brown fox").unwrap();

        let mut hasher = Sha256::new();
        hasher.update(b"the quick brown fox");
        let correct = hex::encode(hasher.finalize());

        assert!(verify_checksum(&path, &correct, "sha256").is_ok());
        assert!(verify_checksum(&path, "deadbeef", "sha256").is_err());
        assert!(verify_checksum(&path, &correct, "md5").is_err());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn install_rejects_bad_checksum_without_extracting() {
        let src = std::env::temp_dir().join(format!("pirat-install-src-{}.zip", uuid::Uuid::new_v4()));
        let file = fs::File::create(&src).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        writer.start_file("game.exe", options).unwrap();
        writer.write_all(b"pretend executable").unwrap();
        writer.finish().unwrap();

        let install_root = std::env::temp_dir().join(format!("pirat-install-root-{}", uuid::Uuid::new_v4()));
        let result = install_downloaded_file(&src, &install_root, "Test Game", Some("0000"), Some("sha256"));
        assert!(result.is_err());
        assert!(!install_root.join("Test Game").exists());

        fs::remove_file(&src).ok();
    }

    #[test]
    fn install_extracts_zip_and_finds_executable() {
        let src = std::env::temp_dir().join(format!("pirat-install-src2-{}.zip", uuid::Uuid::new_v4()));
        let file = fs::File::create(&src).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        writer.start_file("game.exe", options.clone()).unwrap();
        writer.write_all(b"pretend executable").unwrap();
        writer.start_file("readme.txt", options).unwrap();
        writer.write_all(b"hi").unwrap();
        writer.finish().unwrap();

        let install_root = std::env::temp_dir().join(format!("pirat-install-root2-{}", uuid::Uuid::new_v4()));
        let result = install_downloaded_file(&src, &install_root, "Test Game!", None, None).unwrap();
        assert!(result.install_path.ends_with("Test Game_"));
        assert!(result.suggested_executable.is_some());
        assert!(result.suggested_executable.unwrap().ends_with("game.exe"));

        fs::remove_file(&src).ok();
        fs::remove_dir_all(&install_root).ok();
    }
}
