<div align="center">

# <a href="https://alain.xyz/libraries/foil"><img alt="Foil" src="docs/foil-logo.svg" /></a>

[![License][license-img]][license-url]
[![Unit Tests][actions-img]][actions-url]

</div>

✨ Foil is an _content management system_ designed for engineers, artists, technical artists, musicians, and bloggers looking to showcase a portfolio of front-end experiments, games, art, articles, and more.

It's built on top of [Rust](https://www.rust-lang.org/), [Node.js](https://nodejs.org/en), [TypeScript](https://www.typescriptlang.org/), [React](https://reactjs.org/), [GraphQL](https://graphql.org/), and [PostgreSQL](https://www.postgresql.org/).

## Getting started

### 🌟 Installation

Install the following prior to running foil:

- [Node.js](https://nodejs.org) - Handles server-side rendering and JavaScript bundles.

- [PostgreSQL](https://www.postgresql.org/) - Handles the foil post database cache.

- [Rust Language](https://www.rust-lang.org/) (optional) - The language the server and builder are written in. _This isn't necessary if you run foil directly from a binary_.

From there, visit the [releases](/releases) page for built binaries, and expose the folder to your `PATH`.

### ✨ Usage

```bash
# 🛠️ Build your foil project, both the frontend and the portfolio, whatever's changed recently.
foil-cli build # --dev (optional)

# 🏃‍♂️ start the foil server.
foil-cli server start

# ♻️ Reset the database cache.
foil-cli server reset
```

## Environment Variables

| Variable            | Default                     | Description                                             |
| ------------------- | --------------------------- | ------------------------------------------------------- |
| `FOIL_DATABASE_URL` | `postgres://localhost/foil` | What postgres database to write the foil post cache to. |
| `FOIL_HOSTNAME`     | `127.0.0.1`                 | Your hostname. Defaults to localhost.                   |
| `FOIL_PORT`         | `4017`                      | Default port for the foil backend server.               |

[license-img]: https://img.shields.io/:license-mit-blue.svg?style=flat-square
[license-url]: https://opensource.org/license/mit/
[actions-img]: https://img.shields.io/github/actions/workflow/status/alaingalvan/foil/ci.yml?style=flat-square
[actions-url]: https://github.com/alaingalvan/foil/actions/
