CREATE INDEX IF NOT EXISTS posts_permalink_lower_idx
ON posts (LOWER(permalink));
