import pg from "pg";
import { globToRegex } from "./glob.js";
import type { Author, Post } from "./types.js";

const POST_COLUMNS = `id, name, permalink, title, to_json(authors) AS authors, description, keywords, rss, covers,
  date_published, root_path, output_path, public_modules, main`;

interface AuthorRow {
  name?: string;
  email?: string;
  url?: string;
}

interface PostRow {
  id: number;
  name: string;
  permalink: string;
  title: string;
  authors: AuthorRow[] | string;
  description: string;
  keywords: string[];
  rss: string[];
  covers: string[];
  date_published: Date;
  root_path: string;
  output_path: string;
  public_modules: string[];
  main: string;
}

function authors(value: AuthorRow[] | string): Author[] {
  const rows = typeof value === "string" ? (JSON.parse(value) as AuthorRow[]) : value || [];
  return rows.map((row) => ({
    name: row.name || "",
    email: row.email || "",
    url: row.url || "",
  }));
}

function post(row: PostRow): Post {
  return {
    id: row.id,
    name: row.name,
    permalink: row.permalink,
    title: row.title,
    authors: authors(row.authors),
    description: row.description,
    keywords: row.keywords || [],
    rss: row.rss?.length ? row.rss : ["/blog/*"],
    covers: row.covers || [],
    datePublished: new Date(row.date_published),
    rootPath: row.root_path,
    outputPath: row.output_path,
    publicModules: row.public_modules || [],
    main: row.main || "",
  };
}

export function databaseUrl(): string {
  return process.env.FOIL_DATABASE_URL || "postgres://localhost/foil";
}

export async function loadFeedPosts(): Promise<{ root: Post; items: Post[] }> {
  const client = new pg.Client({ connectionString: databaseUrl() });
  await client.connect();
  try {
    const rootResult = await client.query<PostRow>(
      `SELECT ${POST_COLUMNS} FROM posts WHERE permalink = $1`,
      ["/"],
    );
    if (!rootResult.rows[0]) {
      throw new Error("No root post at permalink /. Run foil build first.");
    }
    const root = post(rootResult.rows[0]);
    const items: Post[] = [];
    const seen = new Set<number>();
    for (const glob of root.rss) {
      const found = await client.query<PostRow>(
        `SELECT ${POST_COLUMNS} FROM posts WHERE LOWER(permalink) ~ $1 ORDER BY date_published DESC`,
        [globToRegex(glob)],
      );
      for (const row of found.rows) {
        if (seen.has(row.id) || row.permalink === "/") continue;
        seen.add(row.id);
        items.push(post(row));
      }
    }
    items.sort((a, b) => b.datePublished.getTime() - a.datePublished.getTime());
    return { root, items };
  } finally {
    await client.end();
  }
}


