# foil-rss

Generate an RSS 2.0 feed from the Foil post database. Article posts are rendered to static HTML when their built bundle exports a React component. Everything else keeps the short synopsis.

Run `foil build` first. This tool does not compile posts.

```bash
npm install
npm start
npm test
node dist/main.js generate
```

| Variable | Default | Description |
| --- | --- | --- |
| `FOIL_DATABASE_URL` | `postgres://localhost/foil` | Post cache written by `foil build`. |
| `FOIL_SITE_URL` | author URL, if absolute | Origin used for `link`, `guid`, and relative URLs inside rendered HTML. Required when the author URL is relative. |

The feed is written to `{root output}/rss.xml`. Item `description` stays the synopsis. Rendered HTML goes in `content:encoded`.