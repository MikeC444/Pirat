use rusqlite::{params, Connection, OptionalExtension};

pub fn get_raw(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
        .optional()
}

pub fn set_raw(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn all(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}

pub fn clear_all(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM settings", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn set_get_roundtrip() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get_raw(&conn, "theme").unwrap(), None);
        set_raw(&conn, "theme", "dark").unwrap();
        assert_eq!(get_raw(&conn, "theme").unwrap(), Some("dark".to_string()));
        set_raw(&conn, "theme", "light").unwrap();
        assert_eq!(get_raw(&conn, "theme").unwrap(), Some("light".to_string()));
        assert_eq!(all(&conn).unwrap().len(), 1);
        clear_all(&conn).unwrap();
        assert!(all(&conn).unwrap().is_empty());
    }
}
