-- Save-file locations (from PCGamingWiki) and the newest save seen there,
-- which dates the last session for games no store or tracker saw played.
ALTER TABLE games ADD COLUMN save_paths_json TEXT;
ALTER TABLE games ADD COLUMN save_checked_at INTEGER;
ALTER TABLE games ADD COLUMN save_seen_at INTEGER;
