import {
  readFile as readFileCallback,
  writeFile as writeFileCallback,
} from "fs";
import { join } from "path";
import { promisify } from "util";
import Find from "find";
const { fileSync } = Find;
import { cwd } from "process";

const readFile = promisify(readFileCallback);
const writeFile = promisify(writeFileCallback);

// Find all relative imports in dist/*/*.js files, append .js extension if missing.
async function postBuild() {
  let foundJsFiles = fileSync(/\.js$"/, join(cwd(), "dist"));

  // Enqueue tasks:
  let threads = [];
  for (let foundJsFile of foundJsFiles) {
    let jsFile = await readFile(foundJsFile).toString();
    jsFile.replaceAll(/from ("|')(\.\/.+)("|')/, "from '$2.js'");
    threads.push(writeFile(foundJsFile, jsFile));
  }

  // Clear queue:
  for (let t of threads) {
    await t;
  }
}

postBuild();
