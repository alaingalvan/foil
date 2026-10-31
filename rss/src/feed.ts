import { statSync } from "node:fs";
import { extname, join } from "node:path";
import type { Post } from "./types.js";
import { absoluteUrl } from "./urls.js";
import { cdata, element, escapeXml, rfc822 } from "./xml.js";

export interface FeedItem {
  post: Post;
  html: string;
}

const MIME: Record<string, string> = {
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".png": "image/png",
  ".gif": "image/gif",
  ".webp": "image/webp",
};

function enclosure(post: Post, origin: string): string {
  const cover = post.covers[0];
  if (!cover) return "";
  const url = absoluteUrl(cover, origin);
  if (!url) return "";
  const relative = cover.startsWith(post.permalink)
    ? cover.slice(post.permalink.length).replace(/^[/\\]/, "")
    : cover.replace(/^[/\\]/, "");
  let length = 0;
  try {
    length = statSync(join(post.rootPath, relative)).size;
  } catch {
    return "";
  }
  if (length <= 0) return "";
  const type = MIME[extname(cover).toLowerCase()] || "application/octet-stream";
  return `<enclosure url="${escapeXml(url)}" length="${length}" type="${type}"/>`;
}

function author(post: Post): string {
  const person = post.authors[0];
  if (!person?.email) return "";
  const name = person.name ? ` (${person.name})` : "";
  return element("author", person.email + name);
}

export interface PageLinks {
  self: string;
  first: string;
  previous?: string;
  next?: string;
}

function pageLinks(links: PageLinks | undefined, origin: string): string {
  if (!links) return "";
  return (["self", "first", "previous", "next"] as const)
    .filter((rel) => links[rel])
    .map((rel) => `<atom:link rel="${rel}" type="application/rss+xml" href="${escapeXml(absoluteUrl(links[rel]!, origin))}"/>`)
    .join("\n");
}

export function buildFeed(root: Post, items: FeedItem[], origin: string, links?: PageLinks): string {
  const link = absoluteUrl(root.authors[0]?.url || "/", origin) || origin;
  const categories = root.keywords.map((tag) => element("category", tag)).join("");
  const itemXml = items
    .map(({ post, html }) => {
      const itemLink = absoluteUrl(post.permalink, origin);
      const body = html
        ? `<content:encoded>${cdata(html)}</content:encoded>`
        : "";
      return `<item>
${element("title", post.title)}
${element("link", itemLink)}
${element("description", post.description)}
${element("guid", itemLink, ' isPermaLink="true"')}
${element("pubDate", rfc822(post.datePublished))}
${author(post)}
${post.keywords.map((tag) => element("category", tag)).join("")}
${enclosure(post, origin)}
${body}
</item>`;
    })
    .join("\n");

  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/" xmlns:atom="http://www.w3.org/2005/Atom">
<channel>
${element("title", root.title)}
${pageLinks(links, origin)}
${element("link", link)}
${element("description", root.description)}
${element("language", "en")}
${element("lastBuildDate", rfc822(new Date()))}
${element("generator", "foil-rss")}
${element("docs", "https://www.rssboard.org/rss-specification")}
${categories}
${itemXml}
</channel>
</rss>
`;
}
