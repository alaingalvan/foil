CREATE TYPE author AS (
    name varchar(254),
    email varchar(254),
    url varchar(254)
);

CREATE TABLE IF NOT EXISTS posts (
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

SET CLIENT_ENCODING TO 'utf8';