-- The program a folder game's Play starts, when the user picked another than
-- the one the folder scan chose. Relative to the game's install folder, so it
-- can never point outside it. NULL means the scan's choice.
ALTER TABLE store_entries ADD COLUMN launch_exe TEXT;
