SELECT id, permalink, title, authors, description, keywords, cover, main, date_published, date_modified, root_path
FROM posts
WHERE LOWER(title) LIKE LOWER($1) or LOWER(description) LIKE LOWER($1) or array_to_string(keywords, ',') like LOWER($1)