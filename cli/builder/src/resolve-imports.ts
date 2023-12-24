import { argv } from "process";
import { dirname, isAbsolute, join, relative } from "path";
import { existsSync, readFileSync } from "fs";
import Find from "find";
const { fileSync } = Find;
import { toList } from "dependency-tree";

// 📃 Parse Args:
// resolve-imports <root_path> <file>
// Output is list of JS/TS import files, eg: ["c:/main.js", ...]
if (argv.length < 2) {
  console.error("Missing arguments: resolve-imports <root> <file>");
  process.exit(1);
}
let rootPath = argv[argv.length - 2];
let main = argv[argv.length - 1];

// 🔎 Resolve JavaScript dependencies of main file:
// This only applies to packages outside `node_modules`,
// While it can resolve `import` statements, CommonJS works best.
let resolvedImportSet = new Set<string>();
if (main.match(/\.(t|j)sx?$/)) {
  let filename = main;
  if (!isAbsolute(filename)) {
    filename = join(rootPath, filename);
  }
  if (existsSync(filename)) {
    const addDependencies = (jsFile: string) => {
      let dependencies = toList({
        filename: jsFile,
        directory: rootPath,
        filter: (path) => path.indexOf("node_modules") === -1,
        nodeModulesConfig: {
          entry: "module",
        },
        tsConfig: {
          compilerOptions: {
            target: "es2016",
            module: "CommonJS",
            isolatedModules: true,
            allowSyntheticDefaultImports: true,
            noImplicitAny: false,
            suppressImplicitAnyIndexErrors: true,
            removeComments: true,
            jsx: "react",
          },

          transpileOnly: true,
        },
      });

      // Add files to import set.
      dependencies.forEach((file: string) => {
        resolvedImportSet.add(file);
        // MDX files get special treatment:
        if (/\.mdx$/.test(file) && file != jsFile) {
          // Read the file and resolve all 'import' statements manually.
          // Doesn't cover dynamic import(...) statements is inside a code block.
          let mdxPost = readFileSync(file).toString();
          let importFile = /(import\s*.*)(("|')(.*)("|'))/g;
          let matches = mdxPost.match(importFile);
          for (let m of matches) {
            // Get the './my/path' portion of the import statement.
            let groups = /('|")(.*)('|")/.exec(m);
            let mdxImportFile = groups[2];

            // Build an absolute path from it relative to the current file.
            let filePath = dirname(file);
            let fileNameTest = join(filePath, mdxImportFile);
            fileNameTest = relative(rootPath, fileNameTest);
            let foundMDXImports = fileSync(
              new RegExp(
                "(" +
                  fileNameTest +
                  "(\\/|\\\\)index\\.(j|t)sx?$)|(" +
                  fileNameTest +
                  "\\.(j|t)sx?$)"
              ),
              rootPath
            );
            for (let foundMDXImport of foundMDXImports) {
              addDependencies(foundMDXImport);
            }
          }
        }
      });
    };
    addDependencies(filename);
  }
}

// Return all dependencies.
console.log(JSON.stringify([...resolvedImportSet]));
