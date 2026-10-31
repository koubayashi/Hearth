-- The store's own cover and background, kept even after SteamGridDB or the
-- user replaces them, so the artwork picker can always offer them again.
ALTER TABLE games ADD COLUMN store_cover TEXT;
ALTER TABLE games ADD COLUMN store_hero TEXT;
UPDATE games SET store_cover = cover, store_hero = hero WHERE COALESCE(art_source, 'store') = 'store';
