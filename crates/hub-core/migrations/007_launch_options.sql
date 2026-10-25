-- Extra ways to play a game, added by the user: a script, a mod loader, a
-- server replacement or another executable, alongside the store's own launch.
CREATE TABLE launch_options (
    id          INTEGER PRIMARY KEY,
    game_id     INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    path        TEXT    NOT NULL,
    -- Passed to the program exactly as typed, as in a Windows shortcut.
    args        TEXT    NOT NULL DEFAULT '',
    working_dir TEXT,
    -- What the Play button does. None set means the store's launch.
    is_default  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX launch_options_game ON launch_options (game_id);
