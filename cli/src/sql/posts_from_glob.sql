SELECT id, name, permalink, title, authors, description, keywords, rss, covers, date_published, root_path, output_path, public_modules
FROM posts
WHERE LOWER(permalink) ~ $1
ORDER BY date_published DESC