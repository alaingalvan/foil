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

The feed is paginated, 10 items per page by default. Pass `--page-size <n>` to change it. The newest page is `{root output}/rss.xml`, and older pages are `{root output}/rss/1.xml`, `rss/2.xml`, and so on. Page numbers shift as you publish. The `rss/` folder is created if missing and cleared on every run, so stale pages never linger. Pages link to each other with RFC 5005 `atom:link` (`self`, `first`, `previous`, `next`), assuming the output folder is served at `/assets`. Item `description` stays the synopsis. Rendered HTML goes in `content:encoded`.