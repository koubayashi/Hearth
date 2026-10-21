-- Store-page details (JSON) and when we last looked, so lookups happen once.
ALTER TABLE games ADD COLUMN details_json TEXT;
ALTER TABLE games ADD COLUMN details_checked_at INTEGER;
-- Size of the install folder in bytes, measured on demand.
ALTER TABLE games ADD COLUMN install_size INTEGER;
