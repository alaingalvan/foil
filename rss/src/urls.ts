export function siteOrigin(authorUrl: string, envUrl = process.env.FOIL_SITE_URL): string {
  const candidate = envUrl || authorUrl || "";
  try {
    const url = new URL(candidate);
    if (url.protocol === "http:" || url.protocol === "https:") {
      return url.origin;
    }
  } catch {
    // Relative author URLs need FOIL_SITE_URL.
  }
  return "";
}

export function absoluteUrl(value: string, origin: string): string {
  if (!value) return "";
  try {
    const url = new URL(value);
    if (url.protocol === "http:" || url.protocol === "https:") return url.href;
    return "";
  } catch {
    if (!origin || !value.startsWith("/")) return "";
    return new URL(value, origin.endsWith("/") ? origin : origin + "/").href;
  }
}

/** RSS has no document base. Rewrite relative links before the HTML is embedded. */
export function absolutizeHtml(html: string, origin: string): string {
  if (!origin) return html;
  return html.replace(
    /\s(href|src)=("([^"]*)"|'([^']*)')/gi,
    (match, attr: string, _quoted: string, doubleQuoted?: string, singleQuoted?: string) => {
      const value = doubleQuoted ?? singleQuoted ?? "";
      if (!value || /^(https?:|mailto:|data:|#)/i.test(value)) return match;
      const quote = doubleQuoted !== undefined ? '"' : "'";
      const resolved = value.startsWith("/")
        ? absoluteUrl(value, origin)
        : absoluteUrl("/" + value.replace(/^\.\//, ""), origin);
      return resolved ? ` ${attr}=${quote}${resolved}${quote}` : match;
    },
  );
}
