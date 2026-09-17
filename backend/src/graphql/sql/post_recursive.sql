SELECT root_path, permalink, assets, main
FROM posts
WHERE LOWER(permalink) = ANY($1)
ORDER BY length(permalink) DESC
LIMIT 1