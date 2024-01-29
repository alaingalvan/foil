SELECT id, permalink, title, authors, description, keywords, rss, covers, date_published, root_path, output_path, public_modules FROM posts WHERE permalink LIKE $1
ORDER BY date_published DESC