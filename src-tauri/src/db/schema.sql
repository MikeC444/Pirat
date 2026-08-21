CREATE TABLE IF NOT EXISTS sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    url TEXT NOT NULL UNIQUE,
    enabled INTEGER NOT NULL DEFAULT 1,
    update_interval_minutes INTEGER NOT NULL DEFAULT 60,
    last_updated_at TEXT,
    last_error TEXT
);

CREATE TABLE IF NOT EXISTS games (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    cover_url TEXT,
    version TEXT,
    size_bytes INTEGER,
    release_date TEXT,
    genres_json TEXT NOT NULL DEFAULT '[]',
    developer TEXT,
    publisher TEXT,
    download_url TEXT,
    checksum TEXT,
    checksum_algo TEXT,
    added_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(source_id, external_id)
);

CREATE INDEX IF NOT EXISTS idx_games_title ON games(title);
CREATE INDEX IF NOT EXISTS idx_games_source ON games(source_id);

CREATE TABLE IF NOT EXISTS downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'queued',
    progress_bytes INTEGER NOT NULL DEFAULT 0,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    speed_bps REAL NOT NULL DEFAULT 0,
    started_at TEXT,
    updated_at TEXT NOT NULL,
    error TEXT,
    dest_path TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_downloads_game ON downloads(game_id);

CREATE TABLE IF NOT EXISTS installations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    install_path TEXT NOT NULL,
    executable_path TEXT,
    installed_at TEXT NOT NULL,
    version_installed TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_installations_game ON installations(game_id);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
