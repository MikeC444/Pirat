use crate::models::{GameSource, NewGameSource};
use rusqlite::{params, Connection, OptionalExtension};

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<GameSource>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, url, enabled, update_interval_minutes, last_updated_at, last_error
         FROM sources ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], row_to_source)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<GameSource>> {
    conn.query_row(
        "SELECT id, name, url, enabled, update_interval_minutes, last_updated_at, last_error
         FROM sources WHERE id = ?1",
        params![id],
        row_to_source,
    )
    .optional()
}

pub fn create(conn: &Connection, source: &NewGameSource) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO sources (name, url, enabled, update_interval_minutes) VALUES (?1, ?2, 1, ?3)",
        params![source.name, source.url, source.update_interval_minutes],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_enabled(conn: &Connection, id: i64, enabled: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sources SET enabled = ?1 WHERE id = ?2",
        params![enabled, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sources WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn mark_synced(conn: &Connection, id: i64, timestamp: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sources SET last_updated_at = ?1, last_error = NULL WHERE id = ?2",
        params![timestamp, id],
    )?;
    Ok(())
}

pub fn mark_error(conn: &Connection, id: i64, error: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sources SET last_error = ?1 WHERE id = ?2",
        params![error, id],
    )?;
    Ok(())
}

fn row_to_source(row: &rusqlite::Row) -> rusqlite::Result<GameSource> {
    Ok(GameSource {
        id: row.get(0)?,
        name: row.get(1)?,
        url: row.get(2)?,
        enabled: row.get(3)?,
        update_interval_minutes: row.get(4)?,
        last_updated_at: row.get(5)?,
        last_error: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn create_list_and_delete_roundtrip() {
        let conn = db::open_in_memory().unwrap();
        let id = create(
            &conn,
            &NewGameSource {
                name: "Test Source".into(),
                url: "https://example.com/games.json".into(),
                update_interval_minutes: 30,
            },
        )
        .unwrap();

        let sources = list(&conn).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, id);
        assert!(sources[0].enabled);

        set_enabled(&conn, id, false).unwrap();
        let source = get(&conn, id).unwrap().unwrap();
        assert!(!source.enabled);

        mark_synced(&conn, id, "2026-01-01T00:00:00Z").unwrap();
        let source = get(&conn, id).unwrap().unwrap();
        assert_eq!(source.last_updated_at.as_deref(), Some("2026-01-01T00:00:00Z"));

        delete(&conn, id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}
