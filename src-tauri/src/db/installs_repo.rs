use crate::models::Installation;
use rusqlite::{params, Connection, OptionalExtension, Row};

pub fn upsert(
    conn: &Connection,
    game_id: i64,
    install_path: &str,
    executable_path: Option<&str>,
    version_installed: Option<&str>,
    now: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO installations (game_id, install_path, executable_path, installed_at, version_installed)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(game_id) DO UPDATE SET
            install_path = excluded.install_path,
            executable_path = excluded.executable_path,
            installed_at = excluded.installed_at,
            version_installed = excluded.version_installed",
        params![game_id, install_path, executable_path, now, version_installed],
    )?;
    conn.query_row(
        "SELECT id FROM installations WHERE game_id = ?1",
        params![game_id],
        |row| row.get(0),
    )
}

pub fn get_by_game(conn: &Connection, game_id: i64) -> rusqlite::Result<Option<Installation>> {
    conn.query_row(
        &format!("{SELECT_COLS} WHERE game_id = ?1"),
        params![game_id],
        row_to_install,
    )
    .optional()
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Installation>> {
    let mut stmt = conn.prepare(&format!("{SELECT_COLS} ORDER BY installed_at DESC"))?;
    let rows = stmt.query_map([], row_to_install)?;
    rows.collect()
}

pub fn set_executable(conn: &Connection, game_id: i64, executable_path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE installations SET executable_path = ?1 WHERE game_id = ?2",
        params![executable_path, game_id],
    )?;
    Ok(())
}

pub fn delete_by_game(conn: &Connection, game_id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM installations WHERE game_id = ?1", params![game_id])?;
    Ok(())
}

const SELECT_COLS: &str = "SELECT id, game_id, install_path, executable_path, installed_at, \
     version_installed FROM installations";

fn row_to_install(row: &Row) -> rusqlite::Result<Installation> {
    Ok(Installation {
        id: row.get(0)?,
        game_id: row.get(1)?,
        install_path: row.get(2)?,
        executable_path: row.get(3)?,
        installed_at: row.get(4)?,
        version_installed: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::{games_repo, sources_repo};
    use crate::models::{CatalogueGame, NewGameSource};

    #[test]
    fn upsert_and_lookup() {
        let conn = db::open_in_memory().unwrap();
        let source_id = sources_repo::create(
            &conn,
            &NewGameSource {
                name: "T".into(),
                url: "https://example.com/x.json".into(),
                update_interval_minutes: 60,
            },
        )
        .unwrap();
        let game_id = games_repo::upsert_from_catalogue(
            &conn,
            source_id,
            &CatalogueGame {
                external_id: "g1".into(),
                title: "Game".into(),
                ..Default::default()
            },
            "t",
        )
        .unwrap();

        upsert(&conn, game_id, "/games/game", None, Some("1.0"), "t0").unwrap();
        set_executable(&conn, game_id, "/games/game/game.exe").unwrap();
        let install = get_by_game(&conn, game_id).unwrap().unwrap();
        assert_eq!(install.executable_path.as_deref(), Some("/games/game/game.exe"));

        assert_eq!(list(&conn).unwrap().len(), 1);
        delete_by_game(&conn, game_id).unwrap();
        assert!(get_by_game(&conn, game_id).unwrap().is_none());
    }
}
