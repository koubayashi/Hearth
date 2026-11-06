-- Versions of one game (GTA V and GTA V Enhanced, a game and its remaster)
-- combined into one library entry.

-- The store's own name for each entry, so a version can be named in the
-- panel ("Enhanced") and separated back into its own game with its title.
ALTER TABLE store_entries ADD COLUMN title TEXT;
UPDATE store_entries SET title = (SELECT title FROM games WHERE games.id = store_entries.game_id);

-- Which version a session came from, recorded when games are combined, so
-- separating them gives each its sessions back.
ALTER TABLE sessions ADD COLUMN entry_id INTEGER;

-- Titles of games combined into another: a store reporting that title later
-- (the Enhanced edition bought on a second store) joins the combined game.
CREATE TABLE title_aliases (
    norm_title TEXT PRIMARY KEY,
    game_id    INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE
);
