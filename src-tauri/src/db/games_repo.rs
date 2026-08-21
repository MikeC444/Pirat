use crate::models::{CatalogueGame, Game};
use rusqlite::{params, Connection, OptionalExtension, Row};

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct GameQuery {
    /// Case-insensitive substring match against the title.
    pub search: Option<String>,
    /// If set, only games whose genres include this value are returned.
    pub genre: Option<String>,
    /// "title" | "release_date" | "size_bytes" | "added_at" (default: "title")
    pub sort_by: Option<String>,
    /// "asc" | "desc" (default: "asc")
    pub sort_dir: Option<String>,
    /// Only games that currently belong to an installed source when true.
    pub source_id: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn upsert_from_catalogue(
    conn: &Connection,
    source_id: i64,
    game: &CatalogueGame,
    now: &str,
) -> rusqlite::Result<i64> {
    let genres_json = serde_json::to_string(&game.genres).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO games (
            source_id, external_id, title, description, cover_url, version, size_bytes,
            release_date, genres_json, developer, publisher, download_url, checksum,
            checksum_algo, added_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
        ON CONFLICT(source_id, external_id) DO UPDATE SET
            title = excluded.title,
            description = excluded.description,
            cover_url = excluded.cover_url,
            version = excluded.version,
            size_bytes = excluded.size_bytes,
            release_date = excluded.release_date,
            genres_json = excluded.genres_json,
            developer = excluded.developer,
            publisher = excluded.publisher,
            download_url = excluded.download_url,
            checksum = excluded.checksum,
            checksum_algo = excluded.checksum_algo,
            updated_at = ?15",
        params![
            source_id,
            game.external_id,
            game.title,
            game.description,
            game.cover_url,
            game.version,
            game.size_bytes,
            game.release_date,
            genres_json,
            game.developer,
            game.publisher,
            game.download_url,
            game.checksum,
            game.checksum_algo,
            now,
        ],
    )?;
    conn.query_row(
        "SELECT id FROM games WHERE source_id = ?1 AND external_id = ?2",
        params![source_id, game.external_id],
        |row| row.get(0),
    )
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Game>> {
    conn.query_row(&format!("{SELECT_COLS} WHERE id = ?1"), params![id], row_to_game)
        .optional()
}

pub fn query(conn: &Connection, q: &GameQuery) -> rusqlite::Result<Vec<Game>> {
    let mut sql = SELECT_COLS.to_string();
    let mut conditions: Vec<String> = Vec::new();
    let mut bind_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(search) = q.search.as_ref().filter(|s| !s.trim().is_empty()) {
        conditions.push("title LIKE ?".to_string());
        bind_values.push(Box::new(format!("%{}%", search.trim().replace('%', ""))));
    }
    if let Some(genre) = q.genre.as_ref().filter(|g| !g.trim().is_empty()) {
        conditions.push("genres_json LIKE ?".to_string());
        bind_values.push(Box::new(format!("%\"{}\"%", genre.trim())));
    }
    if let Some(source_id) = q.source_id {
        conditions.push("source_id = ?".to_string());
        bind_values.push(Box::new(source_id));
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    let sort_col = match q.sort_by.as_deref() {
        Some("release_date") => "release_date",
        Some("size_bytes") => "size_bytes",
        Some("added_at") => "added_at",
        Some("updated_at") => "updated_at",
        _ => "title",
    };
    let sort_dir = match q.sort_dir.as_deref() {
        Some("desc") => "DESC",
        _ => "ASC",
    };
    sql.push_str(&format!(" ORDER BY {sort_col} COLLATE NOCASE {sort_dir}"));

    if let Some(limit) = q.limit {
        sql.push_str(" LIMIT ?");
        bind_values.push(Box::new(limit));
        if let Some(offset) = q.offset {
            sql.push_str(" OFFSET ?");
            bind_values.push(Box::new(offset));
        }
    }

    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = bind_values.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(params_refs.as_slice(), row_to_game)?;
    rows.collect()
}

pub fn distinct_genres(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT genres_json FROM games")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut set = std::collections::BTreeSet::new();
    for r in rows {
        let json = r?;
        if let Ok(genres) = serde_json::from_str::<Vec<String>>(&json) {
            for g in genres {
                set.insert(g);
            }
        }
    }
    Ok(set.into_iter().collect())
}

pub fn recently_added(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<Game>> {
    let mut stmt =
        conn.prepare(&format!("{SELECT_COLS} ORDER BY added_at DESC LIMIT ?1"))?;
    let rows = stmt.query_map(params![limit], row_to_game)?;
    rows.collect()
}

const SELECT_COLS: &str = "SELECT id, source_id, external_id, title, description, cover_url, \
     version, size_bytes, release_date, genres_json, developer, publisher, download_url, \
     checksum, checksum_algo, added_at, updated_at FROM games";

fn row_to_game(row: &Row) -> rusqlite::Result<Game> {
    let genres_json: String = row.get(9)?;
    let genres: Vec<String> = serde_json::from_str(&genres_json).unwrap_or_default();
    Ok(Game {
        id: row.get(0)?,
        source_id: row.get(1)?,
        external_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        cover_url: row.get(5)?,
        version: row.get(6)?,
        size_bytes: row.get(7)?,
        release_date: row.get(8)?,
        genres,
        developer: row.get(10)?,
        publisher: row.get(11)?,
        download_url: row.get(12)?,
        checksum: row.get(13)?,
        checksum_algo: row.get(14)?,
        added_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::sources_repo;
    use crate::models::NewGameSource;

    fn setup() -> (Connection, i64) {
        let conn = db::open_in_memory().unwrap();
        let source_id = sources_repo::create(
            &conn,
            &NewGameSource {
                name: "Test".into(),
                url: "https://example.com/a.json".into(),
                update_interval_minutes: 60,
            },
        )
        .unwrap();
        (conn, source_id)
    }

    fn sample(external_id: &str, title: &str, genres: Vec<&str>) -> CatalogueGame {
        CatalogueGame {
            external_id: external_id.into(),
            title: title.into(),
            genres: genres.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn upsert_is_idempotent_and_updates_fields() {
        let (conn, source_id) = setup();
        let id1 = upsert_from_catalogue(&conn, source_id, &sample("g1", "Alpha", vec!["RPG"]), "t1").unwrap();
        let id2 = upsert_from_catalogue(
            &conn,
            source_id,
            &CatalogueGame {
                title: "Alpha Deluxe".into(),
                ..sample("g1", "Alpha", vec!["RPG", "Adventure"])
            },
            "t2",
        )
        .unwrap();
        assert_eq!(id1, id2);
        let game = get(&conn, id1).unwrap().unwrap();
        assert_eq!(game.title, "Alpha Deluxe");
        assert_eq!(game.genres, vec!["RPG", "Adventure"]);
    }

    #[test]
    fn search_filter_and_sort() {
        let (conn, source_id) = setup();
        upsert_from_catalogue(&conn, source_id, &sample("g1", "Zebra Quest", vec!["RPG"]), "t").unwrap();
        upsert_from_catalogue(&conn, source_id, &sample("g2", "Alpha Racer", vec!["Racing"]), "t").unwrap();
        upsert_from_catalogue(&conn, source_id, &sample("g3", "Alpine Adventure", vec!["Adventure"]), "t").unwrap();

        let all = query(&conn, &GameQuery::default()).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].title, "Alpha Racer"); // default sort: title asc

        let searched = query(
            &conn,
            &GameQuery {
                search: Some("alp".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(searched.len(), 2);

        let by_genre = query(
            &conn,
            &GameQuery {
                genre: Some("Racing".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_genre.len(), 1);
        assert_eq!(by_genre[0].title, "Alpha Racer");

        let desc = query(
            &conn,
            &GameQuery {
                sort_dir: Some("desc".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(desc[0].title, "Zebra Quest");
    }
}
