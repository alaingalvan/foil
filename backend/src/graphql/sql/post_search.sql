SELECT id, permalink, title, authors, description, keywords, covers, main, date_published, date_modified
FROM posts
WHERE LOWER(title) LIKE LOWER($1) or LOWER(description) LIKE LOWER($1) or array_to_string(keywords, ',') like LOWER($1)
ORDER BY date_published DESC
LIMIT 10 OFFSET 0