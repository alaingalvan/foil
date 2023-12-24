CREATE TYPE author AS (
    name varchar(254),
    email varchar(254),
    url varchar(254)
);

CREATE TABLE IF NOT EXISTS posts (
    id serial PRIMARY KEY NOT NULL,
    permalink varchar(64) NOT NULL UNIQUE,
    title varchar(254) NOT NULL,
    authors author[] NOT NULL,
    description text NOT NULL,
    keywords varchar(23)[] NOT NULL,
    cover varchar(254) NOT NULL,
    main varchar(254) NOT NULL,
    date_published timestamp NOT NULL,
    date_modified timestamp NOT NULL,
    root_path varchar(254) NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS assets (
    id serial PRIMARY KEY NOT NULL,
    path varchar(254) NOT NULL UNIQUE,
    permalink varchar(254) NOT NULL UNIQUE
);