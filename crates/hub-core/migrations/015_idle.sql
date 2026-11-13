-- Time a game ran while its player was idle (away from the PC, or in another
-- window). Kept apart from sessions, which are play: shown beside the total
-- as "2 h idle" when idle time is kept separate.
CREATE TABLE idle_time (
    id         INTEGER PRIMARY KEY,
    game_id    INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER NOT NULL
);
CREATE INDEX idle_time_game ON idle_time (game_id);

-- One game's own choice: 'count' (idle time counts as play) or 'separate'.
-- NULL follows the setting.
ALTER TABLE games ADD COLUMN idle_mode TEXT;
