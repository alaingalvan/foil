#!/usr/bin/env node
// build.ts

import { exec } from "node:child_process";
import { mkdir, copyFile, cp } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const cargoBuildDir = "/target/release/";

const archMap: Record<string, string> = {
  x64: "x86-64",
  arm64: "arm64",
  ia32: "x86-32",
};

const osName = process.platform;
const gitHashShort = await run("git rev-parse --short HEAD");
const arch = archMap[process.arch] || process.arch;

let gitString: string;
try {
  gitString = await run("git describe --tags --exact-match HEAD");
} catch (_) {
  gitString = gitHashShort;
}

const buildName = `foil-${gitString}-${osName}-${arch}`;
const buildDir = resolve(__dirname, "build", buildName);

console.log(`Building package to: ${buildDir}`);

// ============================================================

async function run(cmd: string, opts: { cwd?: string } = {}): Promise<string> {
  console.log(`> ${cmd}`);
  return new Promise((resolve, reject) => {
    const child = exec(
      cmd,
      { stdio: "inherit", ...opts },
      (err, stdout, _stderr) => {
        if (err) reject(err);
        else resolve(stdout.trim());
      },
    );
    process.on("SIGINT", () => child.kill());
  });
}

// ============================================================

await Promise.all([
  run("cargo build --release", { cwd: "backend" }),
  run("cargo build --release", { cwd: "cli" }),
  run("npm install", { cwd: join(__dirname, "cli/builder") }),
  run("npm start", { cwd: join(__dirname, "cli/builder") }),
  run("npm install", { cwd: join(__dirname, "rss") }),
  run("npm start", { cwd: join(__dirname, "rss") }),
]);

await mkdir(buildDir, { recursive: true });

const binaries = [
  { src: `cli${cargoBuildDir}foil_cli`, dst: "foil_cli" },
  { src: `backend${cargoBuildDir}foil_backend`, dst: "foil_backend" },
];

for (const { src, dst } of binaries) {
  const ext = osName === "win32" ? ".exe" : "";
  await copyFile(resolve(__dirname, src + ext), resolve(buildDir, dst + ext));
  console.log(`Copied ${dst}${ext}`);
}

const builderFiles = [
  { src: `cli${cargoBuildDir}builder/dist`, dst: "builder/dist" },
  { src: `cli${cargoBuildDir}builder/src`, dst: "builder/src" },
  {
    src: `cli${cargoBuildDir}builder/package.json`,
    dst: "builder/package.json",
  },
  {
    src: `cli${cargoBuildDir}builder/package-lock.json`,
    dst: "builder/package-lock.json",
  },
  {
    src: `cli${cargoBuildDir}builder/tsconfig.json`,
    dst: "builder/tsconfig.json",
  },
  { src: "rss/dist", dst: "rss/dist" },
  { src: "rss/src", dst: "rss/src" },
  { src: "rss/package.json", dst: "rss/package.json" },
  { src: "rss/package-lock.json", dst: "rss/package-lock.json" },
  { src: "rss/tsconfig.json", dst: "rss/tsconfig.json" },
];

for (const { src, dst } of builderFiles) {
  if (dst.endsWith(".json") || dst.endsWith(".tsconfig.json")) {
    await copyFile(resolve(__dirname, src), resolve(buildDir, dst));
  } else {
    await mkdir(resolve(buildDir, dst), { recursive: true });
    await cp(resolve(__dirname, src), resolve(buildDir, dst), {
      recursive: true,
    });
  }
  console.log(`Copied ${dst}`);
}

console.log("Build and copy complete.");
