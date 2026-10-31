-- Wikidata lookups learned editions ("GTA V Enhanced" takes GTA V's series),
-- franchises as their own kind, and a search for loose titles ("HITMAN"):
-- ask about every game again. The user's own fixes are kept.
UPDATE games SET wd_checked_at = NULL;
DELETE FROM game_links WHERE source = 'wikidata';
