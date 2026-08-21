use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Extracts a ZIP archive into `dest_dir`, refusing any entry whose path would
/// escape `dest_dir` (a "zip-slip" attack via `../` segments, absolute paths,
/// or Windows drive-letter paths). Untrusted archives are the norm here — the
/// catalogue and its download URLs are attacker-controllable input — so every
/// entry is validated before anything is written to disk.
pub fn extract_zip_safe(archive_path: &Path, dest_dir: &Path) -> Result<usize, String> {
    let file = fs::File::open(archive_path).map_err(|e| format!("failed to open archive: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("not a valid zip archive: {e}"))?;

    fs::create_dir_all(dest_dir).map_err(|e| format!("failed to create install directory: {e}"))?;
    let dest_dir = fs::canonicalize(dest_dir).map_err(|e| format!("failed to resolve install directory: {e}"))?;

    let mut extracted = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("failed to read archive entry {i}: {e}"))?;

        // `enclosed_name()` already rejects absolute paths and `..` components;
        // we still re-verify the resolved path stays under dest_dir below, since
        // defense in depth here is cheap and the cost of getting it wrong is
        // writing outside the install directory.
        let relative = match entry.enclosed_name() {
            Some(p) => p,
            None => {
                // Suspicious entry name (absolute path / traversal) — skip it
                // rather than aborting the whole install.
                continue;
            }
        };

        let out_path = safe_join(&dest_dir, &relative)?;

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| format!("failed to create directory {relative:?}: {e}"))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("failed to create directory for {relative:?}: {e}"))?;
        }

        let mut out_file = fs::File::create(&out_path).map_err(|e| format!("failed to create file {relative:?}: {e}"))?;
        io::copy(&mut entry, &mut out_file).map_err(|e| format!("failed to extract {relative:?}: {e}"))?;
        extracted += 1;
    }

    Ok(extracted)
}

/// Joins `dest_dir` and `relative`, then verifies the result is still inside
/// `dest_dir`. Used as a second line of defense against path traversal even
/// though `enclosed_name()` should already have filtered malicious entries.
fn safe_join(dest_dir: &Path, relative: &Path) -> Result<PathBuf, String> {
    let candidate = dest_dir.join(relative);
    let normalized = normalize_lexically(&candidate);
    if !normalized.starts_with(dest_dir) {
        return Err(format!("archive entry {relative:?} attempts to escape install directory"));
    }
    Ok(normalized)
}

/// Lexically normalizes a path (resolving `.`/`..`) without touching the
/// filesystem, since the target file usually doesn't exist yet so `canonicalize`
/// can't be used on it directly.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn build_zip(entries: &[(&str, &[u8])]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("pirat-archive-test-{}.zip", uuid::Uuid::new_v4()));
        let file = fs::File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        for (name, contents) in entries {
            writer.start_file(*name, options.clone()).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    #[test]
    fn extracts_well_formed_archive() {
        let archive = build_zip(&[("readme.txt", b"hello"), ("bin/game.dat", b"data")]);
        let dest = std::env::temp_dir().join(format!("pirat-extract-{}", uuid::Uuid::new_v4()));

        let count = extract_zip_safe(&archive, &dest).unwrap();
        assert_eq!(count, 2);
        assert_eq!(fs::read_to_string(dest.join("readme.txt")).unwrap(), "hello");
        assert_eq!(fs::read(dest.join("bin/game.dat")).unwrap(), b"data");

        fs::remove_file(&archive).ok();
        fs::remove_dir_all(&dest).ok();
    }

    #[test]
    fn rejects_zip_slip_traversal() {
        // The `zip` crate's own writer refuses to author a traversal entry, so we
        // hand-craft one by writing a raw central-directory-free local file record
        // is overkill for a unit test; instead we validate the guard function
        // directly against a malicious relative path, which is what `by_index`
        // would hand us if `enclosed_name` ever let one slip through.
        let dest = fs::canonicalize(std::env::temp_dir()).unwrap();
        let malicious = Path::new("../../etc/passwd");
        let err = safe_join(&dest, malicious).unwrap_err();
        assert!(err.contains("escape install directory"));
    }
}
