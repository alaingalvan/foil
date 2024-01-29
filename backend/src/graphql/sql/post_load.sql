SELECT id, permalink, title, authors, description, keywords, covers, main, date_published, date_modified FROM posts
WHERE id IN ({})
ORDER BY date_published