SELECT id, permalink, title, authors, description, keywords, covers, main, date_published, date_modified
FROM posts
WHERE LOWER(permalink) ~ $1
ORDER BY CASE WHEN $4 THEN date_modified END DESC NULLS LAST, date_published DESC
LIMIT $2 OFFSET $3