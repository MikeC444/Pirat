pub mod downloads_repo;
pub mod games_repo;
pub mod installs_repo;
pub mod settings_repo;
pub mod sources_repo;

use rusqlite::Connection;
use std::path::Path;

const SCHEMA: &str = include_str!("schema.sql");

/// Opens (creating if necessary) the SQLite database at `path` and applies the schema.
/// Applying `CREATE TABLE IF NOT EXISTS` on every launch acts as our migration runner:
/// safe to re-run, and new tables can be appended to schema.sql over time.
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

/// Opens an in-memory database for tests.
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}
