-- Reset the foil post cache. This is not a migration: `foil server reset` runs it directly.
DROP TABLE IF EXISTS posts;

DROP TYPE IF EXISTS author CASCADE;

DROP TYPE IF EXISTS redirect CASCADE;

CREATE TYPE author AS (
    name varchar(254),
    email varchar(254),
    url varchar(254)
);

CREATE TABLE posts (
    id serial PRIMARY KEY NOT NULL,
    name varchar(254) NOT NULL UNIQUE,
    permalink varchar(254) NOT NULL UNIQUE,
    title varchar(254) NOT NULL,
    authors author[] NOT NULL,
    description text NOT NULL,
    keywords varchar(254)[] NOT NULL,
    covers varchar(254)[] NOT NULL,
    main varchar(254) NOT NULL,
    date_published TIMESTAMPTZ NOT NULL,
    date_modified TIMESTAMPTZ NOT NULL,
    output_path varchar(254) NOT NULL,
    root_path varchar(254) NOT NULL UNIQUE,
    public_modules varchar(254)[] NOT NULL,
    rss varchar(254)[] NOT NULL,
    assets varchar(254)[] NOT NULL
);

CREATE INDEX posts_permalink_lower_idx
ON posts (LOWER(permalink));

SET CLIENT_ENCODING TO 'utf8';
