-- Where the player is with a game: playing, backlog, completed or abandoned.
ALTER TABLE games ADD COLUMN status TEXT;
