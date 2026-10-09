import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import type { Post } from "./types.js";

const RENDERABLE = /\.(mdx|tsx|jsx|ts|js)$/i;
const require = createRequire(import.meta.url);
// Vendor bundles compiled for the Node renderer call require. ESM does not define it.
(globalThis as { require?: NodeRequire }).require = require;

export interface RenderResult {
  html: string;
  reason: string;
}

function fileUrl(path: string): string {
  return "file:///" + path.replaceAll("\\", "/");
}

function modulePath(outputPath: string, specifier: string): string {
  const fileName = specifier.endsWith(".js") ? specifier : specifier + ".js";
  return join(outputPath, fileName);
}

/**
 * Same import map `spawn_renderer` writes: frontend package name plus every
 * public module, all resolved under the root post output path.
 */
export function frontendImportMap(frontend: Post): Record<string, string> {
  const imports: Record<string, string> = {
    [frontend.name]: fileUrl(modulePath(frontend.outputPath, "main")),
  };
  for (const specifier of frontend.publicModules) {
    imports[specifier] = fileUrl(modulePath(frontend.outputPath, specifier));
  }
  return imports;
}

let systemReady: Promise<SystemJSLoader | undefined> | undefined;

function loadSystem(): Promise<SystemJSLoader | undefined> {
  systemReady ??= (async () => {
    const systemPath = require.resolve("systemjs");
    await import(fileUrl(systemPath));
    await import(fileUrl(join(dirname(systemPath), "extras/named-register.js")));
    return (globalThis as { System?: SystemJSLoader }).System;
  })();
  return systemReady;
}

/**
 * Load a built SystemJS post and render its default export.
 * Returns an empty string when the post is not a static React component.
 */
export async function renderPost(post: Post, frontend: Post): Promise<RenderResult> {
  if (!RENDERABLE.test(post.main || "")) {
    return { html: "", reason: "main is not a script or MDX file" };
  }
  const bundle = modulePath(post.outputPath, "main");
  if (!existsSync(bundle)) {
    return { html: "", reason: "main.js has not been built" };
  }

  const system = await loadSystem();
  if (!system) return { html: "", reason: "SystemJS did not register" };
  // Article name must win over the frontend package of the same name. SystemJS
  // keeps the first resolution, so drop it before each import.
  system.delete?.(fileUrl(bundle));
  system.addImportMap({
    imports: {
      ...frontendImportMap(frontend),
      [post.name]: fileUrl(bundle),
    },
  });

  let loaded: { default?: unknown };
  let jsx: Jsx;
  let renderToStaticMarkup: (node: unknown) => string;
  try {
    loaded = await system.import(post.name);
    // Same React the bundle was compiled against. A second copy makes MDX useContext null.
    ({ jsx } = await system.import("react/jsx-runtime"));
    ({ renderToStaticMarkup } = await system.import("react-dom/server"));
  } catch (error) {
    return { html: "", reason: `import failed: ${error}` };
  }
  if (typeof loaded?.default !== "function") {
    return { html: "", reason: "default export is not a component" };
  }
  if (typeof jsx !== "function" || typeof renderToStaticMarkup !== "function") {
    return { html: "", reason: "frontend React render bundles are missing" };
  }

  try {
    const html = renderToStaticMarkup(jsx(loaded.default, {})).trim();
    if (!html) return { html: "", reason: "render produced empty markup" };
    return { html, reason: "" };
  } catch (error) {
    return { html: "", reason: `render failed: ${error}` };
  }
}

type Jsx = (type: unknown, props: object) => unknown;

interface SystemJSLoader {
  addImportMap(map: { imports: Record<string, string> }): void;
  import(specifier: string): Promise<Record<string, unknown>>;
  delete?(id: string): boolean;
}
