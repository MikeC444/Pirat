use crate::models::DownloadTask;
use rusqlite::{params, Connection, OptionalExtension, Row};

pub fn create(conn: &Connection, game_id: i64, dest_path: &str, now: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO downloads (game_id, state, progress_bytes, total_bytes, speed_bps, updated_at, dest_path)
         VALUES (?1, 'queued', 0, 0, 0, ?2, ?3)",
        params![game_id, now, dest_path],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<DownloadTask>> {
    conn.query_row(&format!("{SELECT_COLS} WHERE id = ?1"), params![id], row_to_task)
        .optional()
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<DownloadTask>> {
    let mut stmt = conn.prepare(&format!("{SELECT_COLS} ORDER BY id DESC"))?;
    let rows = stmt.query_map([], row_to_task)?;
    rows.collect()
}

pub fn update_progress(
    conn: &Connection,
    id: i64,
    progress_bytes: i64,
    total_bytes: i64,
    speed_bps: f64,
    now: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE downloads SET progress_bytes = ?1, total_bytes = ?2, speed_bps = ?3, updated_at = ?4,
         state = CASE WHEN state = 'queued' THEN 'downloading' ELSE state END
         WHERE id = ?5",
        params![progress_bytes, total_bytes, speed_bps, now, id],
    )?;
    Ok(())
}

pub fn set_state(conn: &Connection, id: i64, state: &str, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE downloads SET state = ?1, updated_at = ?2 WHERE id = ?3",
        params![state, now, id],
    )?;
    Ok(())
}

pub fn set_started(conn: &Connection, id: i64, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE downloads SET started_at = COALESCE(started_at, ?1) WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn set_error(conn: &Connection, id: i64, error: &str, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE downloads SET state = 'failed', error = ?1, updated_at = ?2 WHERE id = ?3",
        params![error, now, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
    Ok(())
}

const SELECT_COLS: &str = "SELECT id, game_id, state, progress_bytes, total_bytes, speed_bps, \
     started_at, updated_at, error, dest_path FROM downloads";

fn row_to_task(row: &Row) -> rusqlite::Result<DownloadTask> {
    Ok(DownloadTask {
        id: row.get(0)?,
        game_id: row.get(1)?,
        state: row.get(2)?,
        progress_bytes: row.get(3)?,
        total_bytes: row.get(4)?,
        speed_bps: row.get(5)?,
        started_at: row.get(6)?,
        updated_at: row.get(7)?,
        error: row.get(8)?,
        dest_path: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::{games_repo, sources_repo};
    use crate::models::{CatalogueGame, NewGameSource};

    fn setup_game(conn: &Connection) -> i64 {
        let source_id = sources_repo::create(
            conn,
            &NewGameSource {
                name: "T".into(),
                url: "https://example.com/x.json".into(),
                update_interval_minutes: 60,
            },
        )
        .unwrap();
        games_repo::upsert_from_catalogue(
            conn,
            source_id,
            &CatalogueGame {
                external_id: "g1".into(),
                title: "Game".into(),
                ..Default::default()
            },
            "t",
        )
        .unwrap()
    }

    #[test]
    fn lifecycle() {
        let conn = db::open_in_memory().unwrap();
        let game_id = setup_game(&conn);
        let id = create(&conn, game_id, "/tmp/game.zip", "t0").unwrap();

        update_progress(&conn, id, 50, 100, 1024.0, "t1").unwrap();
        let task = get(&conn, id).unwrap().unwrap();
        assert_eq!(task.state, "downloading");
        assert_eq!(task.progress_bytes, 50);

        set_state(&conn, id, "paused", "t2").unwrap();
        assert_eq!(get(&conn, id).unwrap().unwrap().state, "paused");

        set_error(&conn, id, "boom", "t3").unwrap();
        let task = get(&conn, id).unwrap().unwrap();
        assert_eq!(task.state, "failed");
        assert_eq!(task.error.as_deref(), Some("boom"));

        assert_eq!(list(&conn).unwrap().len(), 1);
        delete(&conn, id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}
