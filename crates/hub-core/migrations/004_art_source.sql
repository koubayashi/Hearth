-- Where a game's artwork came from: 'store' (default), 'sgdb' (SteamGridDB,
-- picked automatically) or 'user' (chosen by hand; never replaced).
ALTER TABLE games ADD COLUMN art_source TEXT;
-- Last time SteamGridDB was asked about this game.
ALTER TABLE games ADD COLUMN sgdb_checked_at INTEGER;
