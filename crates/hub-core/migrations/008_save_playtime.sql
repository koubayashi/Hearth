-- The game's own play timer read from its save files, for games no store
-- times (folder games and the like).
ALTER TABLE games ADD COLUMN save_played_secs INTEGER;
-- Save locations are now also read from PCGamingWiki's per-store rows and
-- found by search when the title differs: look every game up again.
UPDATE games SET save_checked_at = NULL;
