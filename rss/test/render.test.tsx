import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { frontendImportMap, renderPost } from "../src/render.ts";
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
    await mkdir(join(output, "react-dom/server"), { recursive: true });
    await writeFile(
      join(output, "main.js"),
      `System.register(["react"], function (exports) {
        var useContext;
        return {
          setters: [function (react) { useContext = react.useContext; }],
          execute: function () {
            exports("default", function Note() {
              if (useContext() !== "same-react") throw new Error("useContext");
              return "article";
            });
          },
        };
      });`,
    );
    await writeFile(
      join(output, "react.js"),
      `System.register([], function (exports) {
        return { execute: function () { exports("useContext", function () { return "same-react"; }); } };
      });`,
    );
    await writeFile(
      join(output, "react/jsx-runtime.js"),
      `System.register([], function (exports) {
        return { execute: function () { exports("jsx", function (type) { return type(); }); } };
      });`,
    );
    await writeFile(
      join(output, "react-dom/server.js"),
      `System.register([], function (exports) {
        return {
          execute: function () {
            if (typeof require !== "function") throw new Error("require is not defined");
            exports("renderToStaticMarkup", function (node) { return "<p>" + node + "</p>"; });
          },
        };
      });`,
    );

    const frontend = post(root, {
      name: "site",
      permalink: "/",
      main: "src/main.tsx",
      publicModules: ["react", "react/jsx-runtime", "react-dom/server"],
    });
    const imports = frontendImportMap(frontend);
    assert.equal(imports.site, `file:///${join(output, "main.js").replaceAll("\\", "/")}`);
    assert.equal(imports["react-dom/server"], `file:///${join(output, "react-dom/server.js").replaceAll("\\", "/")}`);

    const rendered = await renderPost(post(root), frontend);
    assert.equal(rendered.html, "<p>article</p>");

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
