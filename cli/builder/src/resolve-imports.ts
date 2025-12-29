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
const rootPath = argv[argv.length - 2];
const main = argv[argv.length - 1];

// 🔎 Resolve JavaScript dependencies of main file:
// This only applies to packages outside `node_modules`,
// While it can resolve `import` statements, CommonJS works best.
let resolvedImportSet = new Set<string>();

//=============================================================================
function addDependencies(inputFile: string) {
  // Resolve all MDX imports from a given source file:
  if (/\.mdx$/.test(inputFile)) {
    // Read the file and resolve all 'import' statements manually.
    // Doesn't cover dynamic import(...) statements inside a code block.
    let mdxPost = readFileSync(inputFile).toString();
    let importFile = /(import\s*.*)(("|')(.*)("|'))/g;
    let matches = mdxPost.match(importFile) || [];

    for (let match of matches) {
      // Get the './my/path' portion of the import statement.
      let groups = /('|")(.*)('|")/.exec(match);
      if (!groups) continue;

      let mdxImportFile = groups[2];

      // Build an absolute path from it relative to the current file.
      let filePath = dirname(inputFile);
      let fileNameTest = relative(rootPath, join(filePath, mdxImportFile));

      let fileRegex = new RegExp(
        "(" +
          fileNameTest +
          "(\\/|\\\\)index\\.(j|t)sx?$)|(" +
          fileNameTest +
          "\\.(j|t)sx?$)"
      );

      let foundMDXImports = fileSync(fileRegex, rootPath);
      for (let foundMDXImport of foundMDXImports) {
        if (foundMDXImport != inputFile) {
          addDependencies(foundMDXImport);
        }
      }
    }
    resolvedImportSet.add(inputFile);
  } else if (inputFile.match(/\.(t|j)sx?$/)) {
    let dependencies = toList({
      filename: inputFile,
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
      if (/\.mdx$/.test(file) && file != inputFile) {
        addDependencies(file);
      }
      resolvedImportSet.add(file);
    });
    resolvedImportSet.add(inputFile);
  }
}

//=============================================================================
if (main.match(/\.((t|j)s)|(mdx)$/)) {
  let filename = main;
  if (!isAbsolute(filename)) {
    filename = join(rootPath, filename);
  }
  if (existsSync(filename)) {
    resolvedImportSet.add(filename);
    addDependencies(filename);
  }
}

// Return all dependencies.
console.log(JSON.stringify([...resolvedImportSet]));
