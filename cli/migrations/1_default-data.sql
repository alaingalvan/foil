CREATE TYPE author AS (
    name varchar(254),
    email varchar(254),
    url varchar(254)
);

CREATE TYPE redirect AS (
    path_from varchar(254),
    path_to varchar(254)
);

CREATE TABLE IF NOT EXISTS posts (
    id serial PRIMARY KEY NOT NULL,
    permalink varchar(254) NOT NULL UNIQUE,
    title varchar(254) NOT NULL,
    authors author[] NOT NULL,
    description text NOT NULL,
    keywords varchar(254)[] NOT NULL,
    cover varchar(254) NOT NULL,
    main varchar(254) NOT NULL,
    date_published TIMESTAMPTZ NOT NULL,
    date_modified TIMESTAMPTZ NOT NULL,

    output_path varchar(254) NOT NULL,
    root_path varchar(254) NOT NULL UNIQUE,
    public_modules varchar(254)[] NOT NULL,
    rss varchar(254)[] NOT NULL,
    redirects redirect[] NOT NULL
);

CREATE TABLE IF NOT EXISTS assets (
    id serial PRIMARY KEY NOT NULL,
    path varchar(254) NOT NULL UNIQUE,
    permalink varchar(254) NOT NULL UNIQUE
);

SET CLIENT_ENCODING TO 'utf8';