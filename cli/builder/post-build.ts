import { readFile, writeFile, mkdir, cp } from "node:fs/promises";
import { join, resolve } from "path";
import Find from "find";
const { fileSync } = Find;
import { cwd } from "process";
import { promisify } from "node:util";
import { exec as execCallback } from "node:child_process";
const exec = promisify(execCallback);

async function postBuild() {
  // 🔎 Find all relative imports in dist/*/*.js files, append .js extension if missing.
  const distPath = join(cwd(), "dist");
  const foundJsFiles = fileSync(/\.js$/, distPath);
  console.log(
    `🔨 Found ${foundJsFiles.length} files in ${distPath}, fixing relative imports to include .js.\n`
  );

  const jsFilePromises = foundJsFiles.map(async (foundJsFile) => {
    const jsFile = (await readFile(foundJsFile)).toString();
    const updatedContent = jsFile.replaceAll(
      /(import\s*.*)("|')(\.\/.*[^j][^s])("|')/g,
      "$1$2$3.js$4"
    );
    return writeFile(foundJsFile, updatedContent);
  });

  await Promise.all(jsFilePromises);

  // 📄 Copy dist to release folder:
  const releaseTargetPath = resolve(join(cwd(), "..", "target"));
  const releasePaths = [
    join(releaseTargetPath, "release", "builder"),
    join(releaseTargetPath, "debug", "builder"),
  ];

  for (const releasePath of releasePaths) {
    console.log(`🛠️ Updating target build folder at:\n    ${releasePath}`);

    await mkdir(releasePath, { recursive: true });

    const copyPromises = [
      cp(distPath, join(releasePath, "dist"), {
        recursive: true,
      }),
      cp(join(cwd(), "src"), join(releasePath, "src"), { recursive: true }),
      cp(join(cwd(), "package.json"), join(releasePath, "package.json")),
      cp(
        join(cwd(), "package-lock.json"),
        join(releasePath, "package-lock.json")
      ),
      cp(join(cwd(), "tsconfig.json"), join(releasePath, "tsconfig.json")),
    ];

    await Promise.all(copyPromises);
    console.log(
      `📄 Copied 'dist/', 'src/', 'package.json', 'package-lock.json', 'tsconfig.json'.`
    );

    await exec("npm ci", { cwd: releasePath });
    console.log(`📂 Updated node_modules with 'npm ci'.`);
  }
}

postBuild();
