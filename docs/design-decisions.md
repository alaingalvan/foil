Foil was designed to make it quick and easy to build a well design portfolio, handling the boring parts of building a website such as managing builders, server-side rendering, SEO, and letting you focus on writing and authoring content. 

# Inspirations

It's a simple CLI CMS tool similar to other projects such as:

- [GatsbyJS](https://www.gatsbyjs.org/)

- [Jekyll](https://jekyllrb.com/)

- [Wordpress](https://github.com/Automattic/wp-calypso)

# Libraries and Tools

## Backend

The **Backend** is a basic Rust HTTP server that handles serving static assets and the GraphQL API, as well as a Node.js HTTP server that handles server-side rendering of your application. It also features a way to keep the server in sync with this Github repo (**Continuous Integration**) via [Github's repository webhooks](https://developer.github.com/v3/repos/hooks/). Alternatives such as using a git remote are also possible depending on your use case.

- [Rust](https://www.rust-lang.org/) - Primary server language.

- [Axum](https://github.com/tokio-rs/axum) - The Rust HTTP server used for this project.

- [Async GraphQL](https://github.com/async-graphql/async-graphql) - The GraphQL library used to handle queries.

- [SQLx](https://github.com/launchbadge/sqlx) - The SQL library used for basic queries.

- [PostgreSQL](https://www.postgresql.org/) - The current database application.

The Node.js server-side renderer uses:

- [Node](https://nodejs.org/en/) - JavaScript based web server.

- [TypeScript](https://www.typescriptlang.org/) - Typed JavaScript with transpilation support for older versions of the language.

- [React](https://reactjs.org/) - The frontend rendering library of choice.

## Frontend

The **Frontend** of every foil application is a React component.

- [React](https://facebook.github.io/react/) - Front-end framework view framework.

- [Apollo GraphQL Client](https://www.apollographql.com/) - the client library for GraphQL queries.

## Builder

- [TypeScript](http://www.typescriptlang.org/) - Typed JavaScript.

- [Webpack](https://webpack.js.org) - Compilation tool for JavaScript.

- [SystemJS](https://github.com/systemjs/systemjs) - an `import()` polyfill.

- [PostCSS](https://github.com/postcss/postcss) - CSS with postprocessing functions applied to its AST.