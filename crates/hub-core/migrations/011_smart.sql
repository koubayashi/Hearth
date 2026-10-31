-- Series, developers and publishers: what Wikidata says about each game, and
-- the user's own fixes to it, which a refresh never undoes.

-- When Wikidata was last asked about the game, and the item it matched.
ALTER TABLE games ADD COLUMN wd_checked_at INTEGER;
ALTER TABLE games ADD COLUMN wd_item TEXT;

-- A series, franchise or company: a Wikidata id ("Q238015"), or "user:…"
-- for a series the user made.
CREATE TABLE entities (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  -- When its parents were last looked up.
  parents_checked_at INTEGER
);

-- "Is part of": a studio's parent company, a series' franchise.
CREATE TABLE entity_parents (
  child TEXT NOT NULL,
  parent TEXT NOT NULL,
  source TEXT NOT NULL, -- 'wikidata', 'user', or 'removed' (taken away by the user)
  PRIMARY KEY (child, parent)
);

CREATE TABLE game_links (
  game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  role TEXT NOT NULL, -- 'series', 'developer' or 'publisher'
  entity TEXT NOT NULL,
  source TEXT NOT NULL, -- 'wikidata', 'user', or 'removed' (taken away by the user)
  PRIMARY KEY (game_id, role, entity)
);
CREATE INDEX game_links_entity ON game_links (entity);
