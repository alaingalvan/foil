import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { renderPost } from "../src/render.ts";
import type { Post } from "../src/types.ts";

function post(root: string, overrides: Partial<Post> = {}): Post {
  return {
    id: 1,
    name: "note",
    permalink: "/blog/note",
    title: "A Note",
    authors: [],
    description: "Short synopsis",
    keywords: [],
    rss: ["/blog/*"],
    covers: [],
    datePublished: new Date("2026-10-07T00:00:00Z"),
    rootPath: root,
    outputPath: join(root, "public"),
    publicModules: ["react"],
    main: "src/note.mdx",
    ...overrides,
  };
}

describe("render", () => {
  it("renders a SystemJS default export and skips a site shell", async () => {
    const root = join(tmpdir(), "foil-rss-render");
    const output = join(root, "public");
    await mkdir(join(output, "react"), { recursive: true });
    await writeFile(
      join(output, "main.js"),
      `System.register(["react"], function (exports, module) {
        var createElement;
        return {
          setters: [function (react) { createElement = react.createElement; }],
          execute: function () {
            exports("default", function Note() {
              return createElement("p", null, "Full article");
            });
          },
        };
      });`,
    );
    await writeFile(
      join(output, "react.js"),
      `System.register([], function (exports) {
        return {
          execute: function () {
            exports("createElement", function (type, _props, child) {
              return { $$typeof: Symbol.for("react.transitional.element"), type: type, props: { children: child }, key: null, ref: null };
            });
          },
        };
      });`,
    );
    await writeFile(
      join(output, "react/jsx-runtime.js"),
      `System.register([], function (exports) {
        return { execute: function () { exports("jsx", React.createElement); } };
      });`,
    );

    const frontend = post(root, { name: "site", permalink: "/", main: "src/main.tsx" });
    const rendered = await renderPost(post(root), frontend);
    assert.equal(rendered.html, "<p>Full article</p>");

    const shellDir = join(root, "shell");
    await mkdir(shellDir, { recursive: true });
    await writeFile(
      join(shellDir, "main.js"),
      `System.register([], function (exports) {
        return { execute: function () { exports("routes", []); } };
      });`,
    );
    const shell = await renderPost(post(root, { name: "shell", main: "src/main.tsx", outputPath: shellDir }), frontend);
    assert.equal(shell.html, "");
    assert.match(shell.reason, /not a component/);
  });
});
