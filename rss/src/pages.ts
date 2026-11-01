export type FeedPage<T> = {
  file: string;
  items: T[];
  links: { self: string; first: string; previous?: string; next?: string };
};

export const DEFAULT_PAGE_SIZE = 10;

/** Page 0 is `rss.xml` (newest). Page n is `rss/n.xml`, progressively older. */
export function paginate<T>(
  items: T[],
  pageSize: number,
  urlBase = '/assets'
): FeedPage<T>[] {
  if (!Number.isInteger(pageSize) || pageSize < 1) {
    throw new Error('Page size must be a positive integer.');
  }
  const count = Math.max(1, Math.ceil(items.length / pageSize));
  const file = (index: number) =>
    index === 0 ? 'rss.xml' : `rss/${index}.xml`;
  const url = (index: number) => `${urlBase}/${file(index)}`;
  return Array.from({ length: count }, (_, index) => ({
    file: file(index),
    items: items.slice(index * pageSize, (index + 1) * pageSize),
    links: {
      self: url(index),
      first: url(0),
      ...(index > 0 ? { previous: url(index - 1) } : {}),
      ...(index < count - 1 ? { next: url(index + 1) } : {})
    }
  }));
}

/** Reads `--page-size <n>` or `--page-size=<n>`. */
export function parsePageSize(args: string[]): number {
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    let value: string | undefined;
    if (arg === '--page-size') value = args[i + 1];
    else if (arg.startsWith('--page-size='))
      value = arg.slice('--page-size='.length);
    else continue;
    if (!value || !/^\d+$/.test(value) || Number(value) < 1) {
      throw new Error('--page-size must be a positive integer.');
    }
    return Number(value);
  }
  return DEFAULT_PAGE_SIZE;
}
