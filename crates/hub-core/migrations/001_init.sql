CREATE TABLE games (
    id          INTEGER PRIMARY KEY,
    title       TEXT    NOT NULL,
    norm_title  TEXT    NOT NULL,
    favorite    INTEGER NOT NULL DEFAULT 0,
    added_at    INTEGER NOT NULL,
    cover       TEXT,
    hero        TEXT,
    logo        TEXT,
    description TEXT
);
CREATE INDEX games_norm_title ON games (norm_title);

CREATE TABLE store_entries (
    id                  INTEGER PRIMARY KEY,
    game_id             INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    store               TEXT    NOT NULL,
    store_game_id       TEXT    NOT NULL,
    installed           INTEGER NOT NULL DEFAULT 0,
    install_dir         TEXT,
    store_playtime_secs INTEGER,
    -- What the store reported on our first sync; see playtime::merged_secs.
    store_baseline_secs INTEGER,
    store_last_played   INTEGER,
    UNIQUE (store, store_game_id)
);
CREATE INDEX store_entries_game ON store_entries (game_id);

-- Keyed by store identity, not game id, so hiding survives re-syncs and merges.
CREATE TABLE hidden (
    store         TEXT NOT NULL,
    store_game_id TEXT NOT NULL,
    PRIMARY KEY (store, store_game_id)
);

CREATE TABLE sessions (
    id         INTEGER PRIMARY KEY,
    game_id    INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER NOT NULL,
    source     TEXT    NOT NULL
);
CREATE INDEX sessions_game ON sessions (game_id);

CREATE TABLE collections (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE collection_games (
    collection_id INTEGER NOT NULL REFERENCES collections (id) ON DELETE CASCADE,
    game_id       INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    PRIMARY KEY (collection_id, game_id)
);
