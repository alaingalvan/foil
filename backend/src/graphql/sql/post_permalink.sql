SELECT id, permalink, title, authors, description, keywords, cover, main, date_published, date_modified, root_path
FROM posts
WHERE LOWER(permalink) LIKE LOWER($1)