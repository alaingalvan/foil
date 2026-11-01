#!/usr/bin/env node
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { loadFeedPosts } from './db';
import { buildFeed } from './feed';
import { paginate, parsePageSize } from './pages';
import { renderPost } from './render';
import { absolutizeHtml, siteOrigin } from './urls';

async function generate(pageSize: number): Promise<void> {
  const { root, items } = await loadFeedPosts();
  const origin = siteOrigin(root.authors[0]?.url || '');
  if (!origin) {
    console.error(
      'Set FOIL_SITE_URL to an absolute http(s) origin. RSS links cannot be relative.'
    );
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
      html: absolutizeHtml(result.html, origin)
    });
  }

  const pages = paginate(rendered, pageSize);

  // Page numbers shift as posts are published, so drop old pages first.
  const rssDir = join(root.outputPath, 'rss');
  await mkdir(root.outputPath, { recursive: true });
  await rm(rssDir, { recursive: true, force: true });
  await mkdir(rssDir, { recursive: true });

  for (const page of pages) {
    const output = join(root.outputPath, page.file);
    await writeFile(
      output,
      buildFeed(root, page.items, origin, page.links),
      'utf8'
    );
    console.log(`Wrote ${output}`);
  }
}

generate(parsePageSize(process.argv.slice(2))).catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
