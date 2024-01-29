import { join, resolve, parse } from "path";
import { argv } from "process";
import {
  stat as statCallback,
  mkdir as mkdirCallback,
  writeFile as writeFileCallback,
} from "fs";
import { promisify } from "util";
import chalk from "chalk";
const { gray, green, cyan, red } = chalk;
import webpack, { Configuration, Compiler } from "webpack";
const { DefinePlugin } = webpack;
import MiniCssExtractPlugin from "mini-css-extract-plugin";

import rehypeKatex from "rehype-katex";
import rehypeSlug from "rehype-slug";

import remarkCode from "./misc/remark-code";
import remarkMath from "remark-math";
import remarkGfm from "remark-gfm";

const mkdir = promisify(mkdirCallback);
const writeFile = promisify(writeFileCallback);
const stat = promisify(statCallback);

//=====================================================================================================================
// 📃 Parse Args:
// foil-builder --root-path <path> --system --public-modules <list> --vendor --input <main> --output <dir>
// Output is empty when successful, an error message when failing.

const kebab = (str) =>
  str
    .replace(/\-/, "") //replace all special chars
    .replace(/\s\s+/, " ") //replace multi-spaces
    .replace(/([a-z])([A-Z])/g, "$1-$2") // get all lowercase letters that are near to uppercase ones
    .replace(/[\s_]+/g, "-") // replace all spaces and low dash
    .toLowerCase(); // convert to lower case

let args = {
  // Main name,
  name: "",
  // Main title
  mainTitle: "",
  // Current build root folder.
  rootPath: "",
  // Input bundle file.
  input: "",
  // Output bundle file name.
  output: "",
  // Build the systemJS runtime.
  system: false,
  // Build the import map and expose it in the output folder.
  inputMap: false,
  // Public modules accessible by other foil packages.
  publicModules: [],
  // Build vendor modules.
  vendor: false,
  // Build in production mode.
  production: false || process?.env["NODE_ENV"]?.match(/production/) != null,
};
for (let i = 0; i < argv.length; i++) {
  let arg = argv[i];
  // Arg matches --args map:
  let found: string = Object.keys(args).reduce(
    (p, c) => ("--" + kebab(c) === arg ? c : p),
    ""
  );
  if (found.length > 0) {
    if (Array.isArray(args[found])) {
      // Look ahead until we find another argument or we're out of arguments.
      let lookForward = true;
      while (lookForward) {
        i++;
        if (i < argv.length) {
          let notFoundForward = !Object.keys(args).reduce(
            (p, c) => p || ("--" + kebab(c) === argv[i] ? true : false),
            false
          );
          if (notFoundForward) {
            args[found].push(argv[i]);
          }
          lookForward = lookForward && notFoundForward && i < argv.length;
          if (!lookForward) i--;
        } else {
          lookForward = false;
        }
      }
    } else if (typeof args[found] === "string") {
      if (i + 1 < argv.length) {
        args[found] = argv[i + 1];
      }
    } else if (typeof args[found] == "boolean") {
      args[found] = true;
    }
  }
}

// Fail if missing required arguments:
if (args.rootPath.length <= 0) {
  console.error(
    "foil-builder --root-path <path> --system --vendor --input <main> --output <dir>"
  );
  process.exit(1);
}
//=====================================================================================================================
// Setup globals:
const nodeEnvStr: any = args.production ? "production" : "development";
const buildDir = args.output;
const buildDirAbs = join(args.rootPath, buildDir);
// TODO: Load join(args.rootPath, "tsconfig.json"), then root foil's tsconfig, then builder's tsconfig.
const tsConfigFile = join(process.cwd(), "tsconfig.json");
const mainTitle =
  args.mainTitle.length < 1 ? parse(args.input).name : args.mainTitle;
const libraryName = args.name.length < 1 ? kebab(mainTitle) : args.name;

//=====================================================================================================================
// 🔧 Build or watch Webpack compilation with some helpful metadata shared.
function build(title, config: Configuration) {
  const buildTitle = title + gray(` (${nodeEnvStr}) `);
  const compiler: Compiler = webpack(config);
  return new Promise((res, rej) => {
    const webpackCallback = (err, stats) => {
      if (err) {
        console.error(err);
        return rej();
      }

      if (stats.hasErrors()) {
        let statsJson = stats.toJson();
        console.log(
          "❌" + red(" · Error · ") + buildTitle + " failed to compile:"
        );
        for (let error of statsJson.errors) {
          console.warn(error.message);
        }
        return rej();
      }
      console.log(
        "✔️️" +
          green("  · Success · ") +
          buildTitle +
          " built in " +
          cyan(+stats.endTime - +stats.startTime + " ms.")
      );
      return res(stats);
    };
    compiler.run(webpackCallback);
  });
}

//=====================================================================================================================
// 🔧 Main builder.
async function main() {
  // ⚙️ Build Import Map
  if (args.inputMap) {
    const importMap = join(buildDirAbs, "importmap.json");
    const importMapData = `{
  "imports": {
    "${libraryName}": "${
      join(args.output, "main").replace(/\\/g, "/") + ".js"
    }"${args.publicModules.length > 0 ? "," : ""}
${args.publicModules.reduce(
  (acc, m, i) =>
    acc +
    '    "' +
    m +
    '": "' +
    (join(args.output, m).replace(/\\/g, "/") + ".js") +
    '"' +
    (i < args.publicModules.length - 1 ? "," : "") +
    "\n",
  ""
)}
  }
}`;
    // Make the output dir if it doesn't already exist:
    try {
      await stat(buildDirAbs);
    } catch (e) {
      await mkdir(buildDirAbs, { recursive: true });
    }

    await writeFile(importMap, importMapData, "utf8");

    console.log(
      "✔️️" +
        green("  · Success · ") +
        " ⚙️ SystemJS input map built to:\n" +
        importMap
    );
  }

  // 🌄 Build SystemJS runtime:
  if (args.system) {
    let suffix = args.production ? ".min" : "";
    await build("🌄 SystemJS", {
      mode: nodeEnvStr,
      entry: {
        system: [
          "systemjs/dist/system" + suffix + ".js",
          "systemjs/dist/extras/named-register" + suffix + ".js",
          "systemjs/dist/extras/dynamic-import-maps" + suffix + ".js",
        ],
      },
      output: {
        path: buildDirAbs,
        filename: "[name].js",
      },
      devtool: args.production ? false : "inline-source-map",
      optimization: {
        minimize: args.production,
      },
    });
  }

  // 📚 Build vendor libraries:
  if (args.publicModules.length > 0 && args.vendor) {
    for (let m of args.publicModules) {
      let web = /\/((server)|(ssr))/.exec(m) === null;
      let externals = args.publicModules.reduce(
        (prev, cur) => (m != cur ? { ...prev, [cur]: cur } : prev),
        {}
      );
      await build(`📚 ${m} (${web ? "web" : "server"})`, {
        mode: nodeEnvStr,
        target: web ? "web" : "node",
        entry: m,
        output: {
          path: buildDirAbs,
          filename: m + ".js",
          libraryTarget: "system",
          library: {
            type: "system",
            name: m,
          },
        },
        resolve: {
          extensions: [".ts", ".tsx", ".js"],
          modules: [
            resolve(args.rootPath, "src"),
            join(args.rootPath, "node_modules"),
            join(process.cwd(), "node_modules"),
            "node_modules",
          ],
          fallback: {
            fs: false,
            tls: false,
            net: false,
            path: false,
            zlib: false,
            http: false,
            https: false,
            stream: false,
            crypto: false,
          },
        },

        module: {
          rules: [
            // React imports can behave strangely when mixing commonJS with ES modules
            // with default imports. This explicitly defines a default import as a wildcard.
            {
              test: /\.(j|t)sx?$/,
              loader: "string-replace-loader",
              options: {
                multiple: [
                  {
                    search: /import\s+React\s+from\s+('|")react('|")/i,
                    replace: "import * as React from 'react'",
                  },
                  {
                    search:
                      /(import\s+React,\s+){(\s*.*\s*)}\s*(from\s+('|")react('|"))/,
                    replace: (_match, _p1, p2) =>
                      `import * as React from 'react';const {${p2}} = React`,
                    flags: "g",
                  },
                ],
              },
            },
          ],
        },
        externalsType: "system",
        externals,
        externalsPresets: {
          web,
        },
        devtool: args.production ? false : "inline-source-map",
        plugins: [
          new DefinePlugin({
            "process.env": {
              NODE_ENV: JSON.stringify(nodeEnvStr),
            },
          }),
        ],
        optimization: {
          minimize: args.production ? true : false,
        },
      });
    }
  }

  // ✨ Build main target:
  if (args.input.length > 0) {
    const relativeInputPath = "./" + args.input;
    await build("✨ " + mainTitle, {
      mode: nodeEnvStr,
      context: args.rootPath,
      entry: {
        main: relativeInputPath,
      },
      output: {
        path: buildDirAbs,
        filename: "[name].js",
        libraryTarget: "system",
        library: { type: "system", name: libraryName },
      },
      externalsPresets: {
        web: true,
      },
      externalsType: "system",
      externals: args.publicModules,
      resolve: {
        extensions: [".ts", ".tsx", ".js"],
        modules: [
          resolve(args.rootPath, "src"),
          join(args.rootPath, "node_modules"),
          join(process.cwd(), "node_modules"),
          "node_modules",
        ],
        fallback: {
          fs: false,
          tls: false,
          net: false,
          path: false,
          zlib: false,
          http: false,
          https: false,
          stream: false,
          crypto: false,
          buffer: false,
        },
      },
      module: {
        rules: [
          {
            test: /\.tsx?$/,
            loader: "ts-loader",
            options: {
              transpileOnly: true,
              configFile: tsConfigFile,
              compilerOptions: {
                module: "esnext",
                sourceMap: args.production ? false : true,
              },
            },
          },
          {
            test: /\.s?css$/,
            use: [
              {
                loader: MiniCssExtractPlugin.loader,
              },
              {
                loader: "css-loader",
              },
              {
                loader: "postcss-loader",
                options: {
                  postcssOptions: {
                    plugins: [
                      "postcss-nested",
                      "postcss-advanced-variables",
                      "postcss-preset-env",
                      "autoprefixer",
                    ],
                  },
                },
              },
            ],
          },
          {
            test: [
              /\.(woff|ttf|eot|svg)(\?v=[a-z0-9]\.[a-z0-9]\.[a-z0-9])?$/,
              /\.(ttf|eot|svg)(\?v=[0-9]\.[0-9]\.[0-9])?$/,
            ],
            type: "asset/resource",
          },
          {
            test: /\.(wgsl|glsl)$/,
            type: "asset/source",
          },
          {
            test: /\.wasm$/,

            loader: "file-loader",
            options: {
              name: "[name].[ext]",
            },
          },
          {
            test: /\.mdx$/,
            use: [
              {
                loader: "ts-loader",
                options: {
                  transpileOnly: true,
                  // At the moment, we only use the `tsconfig.json` in the builder.
                  // Users may want to use the one in their project build directory, so we should prioritize that.
                  configFile: join(process.cwd(), "tsconfig.json"),
                  compilerOptions: {
                    module: "esnext",
                    sourceMap: args.production ? false : true,
                  },
                },
              },
              {
                loader: "@mdx-js/loader",
                options: {
                  jsx: false,
                  providerImportSource: "@mdx-js/react",
                  remarkPlugins: [remarkMath, remarkCode, remarkGfm],
                  rehypePlugins: [rehypeKatex, rehypeSlug],
                },
              },
            ],
          },
        ],
      },
      resolveLoader: {
        modules: ["node_modules", join(process.cwd(), "node_modules")],
        extensions: [".js", ".json"],
        mainFields: ["loader", "main"],
      },
      plugins: [
        new DefinePlugin({
          "process.env": {
            NODE_ENV: JSON.stringify(nodeEnvStr),
          },
        }),
        new MiniCssExtractPlugin({
          filename: "[name].css",
        }) as any,
      ],
      devtool: args.production ? false : "inline-source-map",
      optimization: {
        minimize: args.production ? true : false,
      },
    });
  }

  process.exit(0);
}

main();
