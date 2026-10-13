import chalk from 'chalk';
import { parseArgs } from './args.ts';
import { buildFoil } from './builder/foil.ts';

// Fallback mode: parse argv, build one foil, exit.
export async function runOnce(argv: string[]) {
  const args = parseArgs(argv);
  if (args.rootPath.length <= 0) {
    console.error(
      'foil-builder --root-path <path> --system --vendor --input <main> --output <dir>'
    );
    process.exit(1);
  }
  try {
    await buildFoil(args);
  } catch (e: any) {
    console.error(chalk.red('❌ Error: ') + String(e?.message ?? e));
    process.exit(1);
  }
  process.exit(0);
}
