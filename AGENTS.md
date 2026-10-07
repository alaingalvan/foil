# Foil

This repo is not one project. The root has no `Cargo.toml` and no `package.json`. Do not run `cargo`, `npm`, or `tsc` from the repo root.

There are three independent projects. Change and build only the one the task touches.

| Project | Path | What it is | Build |
| --- | --- | --- | --- |
| CLI | [cli](cli) | Rust binary `foil_cli`. Builds a foil site, watches it, and starts the server. | `cargo check --manifest-path cli/Cargo.toml` |
| Builder | [cli/builder](cli/builder) | Node package `@foil-cms/builder`. Webpack/TypeScript bundler invoked by the CLI. | `npm start` in `cli/builder` |
| Backend | [backend](backend) | Rust binary `foil_backend`. Axum/GraphQL server. Separate crate, separate `target/`. | `cargo check --manifest-path backend/Cargo.toml` |

`cli/src/server.rs` is not the backend. It is CLI code that spawns `foil_backend` and a Node renderer.

## Do not mix these

- `cli` and `backend` are separate Cargo crates. A change in one does not compile the other. Do not add a workspace `Cargo.toml` unless asked.
- `cli/builder` is not part of the CLI crate. Rust does not compile it. The CLI shells out to its `dist/`.
- Root [tsconfig.json](tsconfig.json) and [build.js](build.js) are not the builder. `build.js` only packages release binaries. The builder uses [cli/builder/tsconfig.json](cli/builder/tsconfig.json) and `npm start`.
- `build/` and every `target/` are outputs. Do not edit them.

## Tests

These are binaries, not libraries. `--lib` will not find tests. `--offline` is fine when dependencies are already cached.

```bash
cargo test --manifest-path cli/Cargo.toml --bin foil_cli
cargo test --manifest-path backend/Cargo.toml --bin foil_backend
```

CI builds the three projects as separate jobs. See [.github/workflows/ci.yml](.github/workflows/ci.yml).

## Where to edit

- CLI commands, build, watch, database cache, RSS: [cli/src](cli/src). `cli/src/builder` is a Rust module, not [cli/builder](cli/builder).
- Webpack, import maps, SystemJS, MDX bundling: [cli/builder/src](cli/builder/src).
- HTTP, GraphQL, static routing, reverse proxy to the Node renderer: [backend/src](backend/src).

Product usage is in [readme.md](readme.md). Design notes are in [docs/design-decisions.md](docs/design-decisions.md).
