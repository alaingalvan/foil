SELECT root_path, permalink, assets, main
FROM posts
WHERE LOWER(permalink) = LOWER($1)