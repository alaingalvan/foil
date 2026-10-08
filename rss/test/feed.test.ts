import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { buildFeed } from "../src/feed.ts";
import { globToRegex } from "../src/glob.ts";
import type { Post } from "../src/types.ts";
import { absoluteUrl, absolutizeHtml, siteOrigin } from "../src/urls.ts";
import { cdata, rfc822 } from "../src/xml.ts";

function post(overrides: Partial<Post> = {}): Post {
  return {
    id: 1,
    name: "note",
    permalink: "/blog/note",
    title: "A Note",
    authors: [{ name: "Ada", email: "ada@example.com", url: "https://example.com" }],
    description: "Short synopsis",
    keywords: ["graphics"],
    rss: ["/blog/*"],
    covers: [],
    datePublished: new Date("2026-10-07T00:00:00Z"),
    rootPath: "E:/site",
    outputPath: "public",
    publicModules: ["react"],
    main: "src/note.mdx",
    ...overrides,
  };
}

describe("glob", () => {
  it("escapes permalink punctuation and expands stars", () => {
    assert.equal(globToRegex("/blog/*"), "^\\/blog\\/.*$");
  });
});

describe("urls", () => {
  it("prefers FOIL_SITE_URL over a relative author url", () => {
    assert.equal(siteOrigin("/", "https://alain.xyz"), "https://alain.xyz");
  });

  it("resolves site-relative links and leaves absolute ones", () => {
    assert.equal(absoluteUrl("/blog/note", "https://alain.xyz"), "https://alain.xyz/blog/note");
    assert.equal(absoluteUrl("https://cdn.example/a.png", "https://alain.xyz"), "https://cdn.example/a.png");
  });

  it("rewrites relative html urls", () => {
    const html = absolutizeHtml('<img src="/covers/a.png"><a href="mailto:ada@example.com">mail</a>', "https://alain.xyz");
    assert.match(html, /src="https:\/\/alain\.xyz\/covers\/a\.png"/);
    assert.match(html, /href="mailto:ada@example.com"/);
  });
});

describe("feed", () => {
  it("formats dates in the reliable RFC 822 GMT form", () => {
    assert.equal(rfc822(new Date("2026-10-07T00:00:00Z")), "Wed, 07 Oct 2026 00:00:00 GMT");
  });

  it("keeps the synopsis and puts rendered html in content:encoded", () => {
    const xml = buildFeed(post({ permalink: "/" }), [{ post: post(), html: "<p>Full article</p>" }], "https://example.com");
    assert.match(xml, /<description>Short synopsis<\/description>/);
    assert.ok(xml.includes(`<content:encoded>${cdata("<p>Full article</p>")}</content:encoded>`));
    assert.match(xml, /<guid isPermaLink="true">https:\/\/example\.com\/blog\/note<\/guid>/);
    assert.match(xml, /<author>ada@example\.com \(Ada\)<\/author>/);
  });

  it("omits content when rendering was skipped", () => {
    const xml = buildFeed(post({ permalink: "/" }), [{ post: post(), html: "" }], "https://example.com");
    assert.doesNotMatch(xml, /content:encoded/);
  });
});
