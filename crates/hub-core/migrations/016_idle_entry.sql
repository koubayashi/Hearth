-- Which version idle time came from, recorded when games are combined (as
-- for sessions), so separating a version takes its idle time back too.
ALTER TABLE idle_time ADD COLUMN entry_id INTEGER;
