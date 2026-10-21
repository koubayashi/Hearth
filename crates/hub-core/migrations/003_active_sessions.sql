-- Sessions still running, saved every minute so a crash, kill or shutdown
-- mid-game loses at most a minute. Turned into real sessions on next start.
CREATE TABLE active_sessions (
    game_id    INTEGER PRIMARY KEY REFERENCES games (id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    last_seen  INTEGER NOT NULL
);
