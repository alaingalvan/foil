import {
  readFile as readFileCallback,
  writeFile as writeFileCallback,
  mkdir as mkdirCallback,
  cp as cpCallback,
} from "fs";
import { join, resolve } from "path";
import { promisify } from "util";
import Find from "find";
const { fileSync } = Find;
import { cwd } from "process";

const readFile = promisify(readFileCallback);
const writeFile = promisify(writeFileCallback);
const mkdir = promisify(mkdirCallback);
const cp: any = promisify(cpCallback);

async function postBuild() {
  // Find all relative imports in dist/*/*.js files, append .js extension if missing.
  let distPath = join(cwd(), "dist");
  let foundJsFiles = fileSync(/\.js$/, distPath);
  console.log(
    `🔨 Found ${foundJsFiles.length} files in ${distPath}, fixing imports to include .js.`
  );
  let threads = [];
  for (let foundJsFile of foundJsFiles) {
    let jsFile = (await readFile(foundJsFile)).toString();
    jsFile.replaceAll(/(import\s*.*)("|')(\.\/.*)("|')/g, "$1$2$3.js$4");
    threads.push(writeFile(foundJsFile, jsFile));
  }
  do {
    await threads.pop().catch((e) => console.log(e));
  } while (threads.length > 0);

  // Copy dist to release folder:
  let releaseTargetPath = resolve(join(cwd(), "..", "target"));
  let releasePaths = [
    join(releaseTargetPath, "release", "builder"),
    join(releaseTargetPath, "debug", "builder"),
  ];
  for (let releasePath of releasePaths) {
    await mkdir(releasePath).catch((e) => {});
    threads.push(cp(distPath, join(releasePath, "dist"), { recursive: true }));
    threads.push(
      cp(join(cwd(), "src"), join(releasePath, "src"), { recursive: true })
    );
    threads.push(
      cp(join(cwd(), "package.json"), join(releasePath, "package.json"))
    );
    threads.push(
      cp(
        join(cwd(), "package-lock.json"),
        join(releasePath, "package-lock.json")
      )
    );
    threads.push(
      cp(join(cwd(), "tsconfig.json"), join(releasePath, "tsconfig.json"))
    );
    console.log(
      "📄 Copied 'dist/', 'src/', 'package.json', 'package-lock.json', 'tsconfig.json' to:\n" +
        releasePath
    );
  }
  do {
    await threads.pop().catch((e) => console.log(e));
  } while (threads.length > 0);
}

postBuild();
