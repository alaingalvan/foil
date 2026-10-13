#!/usr/bin/env node
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { loadFeedPosts } from "./db.js";
import { buildFeed } from "./feed.js";
import { renderPost } from "./render.js";
import { absolutizeHtml, siteOrigin } from "./urls.js";

async function generate(): Promise<void> {
  const { root, items } = await loadFeedPosts();
  const origin = siteOrigin(root.authors[0]?.url || "");
  if (!origin) {
    console.error("Set FOIL_SITE_URL to an absolute http(s) origin. RSS links cannot be relative.");
    process.exitCode = 1;
    return;
  }

  const rendered = [];
  for (const item of items) {
    const result = await renderPost(item, root);
    if (result.html) {
      console.log(`Rendered ${item.permalink}`);
    } else {
      console.log(`Synopsis only for ${item.permalink}: ${result.reason}`);
    }
    rendered.push({
      post: item,
      html: absolutizeHtml(result.html, origin),
    });
  }

  const xml = buildFeed(root, rendered, origin);

  await mkdir(root.outputPath, { recursive: true });
  const output = join(root.outputPath, "rss.xml");
  await writeFile(output, xml, "utf8");

  console.log(`Wrote ${output}`);
}

generate().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
