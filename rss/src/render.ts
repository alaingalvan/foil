import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { createElement, isValidElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { Post } from "./types.js";

const RENDERABLE = /\.(mdx|tsx|jsx|ts|js)$/i;
const require = createRequire(import.meta.url);

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
  const reactBundle = modulePath(frontend.outputPath, "react");
  const jsxBundle = modulePath(frontend.outputPath, "react/jsx-runtime");
  if (!existsSync(reactBundle) || !existsSync(jsxBundle)) {
    return { html: "", reason: "frontend React bundles are missing" };
  }

  const imports: Record<string, string> = {
    [post.name]: fileUrl(bundle),
    react: fileUrl(reactBundle),
    "react/jsx-runtime": fileUrl(jsxBundle),
  };
  for (const specifier of post.publicModules) {
    const built = modulePath(frontend.outputPath, specifier);
    if (existsSync(built)) imports[specifier] = fileUrl(built);
  }

  const systemPath = require.resolve("systemjs");
  await import(fileUrl(systemPath));
  await import(fileUrl(join(dirname(systemPath), "extras/named-register.js")));
  const system = (globalThis as { System?: SystemJSLoader }).System;
  if (!system) return { html: "", reason: "SystemJS did not register" };
  system.addImportMap({ imports });

  let loaded: { default?: unknown };
  try {
    loaded = await system.import(post.name);
  } catch (error) {
    return { html: "", reason: `import failed: ${error}` };
  }
  if (typeof loaded?.default !== "function") {
    return { html: "", reason: "default export is not a component" };
  }

  try {
    const node = createElement(loaded.default as Parameters<typeof createElement>[0]);
    if (!isValidElement(node)) return { html: "", reason: "default export did not create an element" };
    const html = renderToStaticMarkup(node).trim();
    if (!html) return { html: "", reason: "render produced empty markup" };
    return { html, reason: "" };
  } catch (error) {
    return { html: "", reason: `render failed: ${error}` };
  }
}

interface SystemJSLoader {
  addImportMap(map: { imports: Record<string, string> }): void;
  import(specifier: string): Promise<{ default?: unknown }>;
}
