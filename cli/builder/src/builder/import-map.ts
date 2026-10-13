import { join } from 'path';
import { stat, mkdir, writeFile } from 'fs/promises';
import chalk from 'chalk';
const { green } = chalk;
import type { BuildArgs } from '../args.ts';

export async function writeImportMap(
  args: BuildArgs,
  libraryName: string,
  buildDirAbs: string
) {
  const importMap = join(buildDirAbs, 'importmap.json');
  const importMapData = `{
  "imports": {
    "${libraryName}": "${
      join(args.output, 'main').replace(/\\/g, '/') + '.js'
    }"${args.publicModules.length > 0 ? ',' : ''}
${args.publicModules.reduce(
  (acc, m, i) =>
    acc +
    '    "' +
    m +
    '": "' +
    (join(args.output, m).replace(/\\/g, '/') + '.js') +
    '"' +
    (i < args.publicModules.length - 1 ? ',' : '') +
    '\n',
  ''
)}
  }
}`;
  // Make the output dir if it doesn't already exist:
  try {
    await stat(buildDirAbs);
  } catch (e) {
    await mkdir(buildDirAbs, { recursive: true });
  }

  await writeFile(importMap, importMapData, 'utf8');

  console.log(
    '✔️️' +
      green('  · Success · ') +
      ' ⚙️ SystemJS input map built to:\n' +
      importMap
  );
}
