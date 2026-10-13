import { join, parse } from 'path';
import { kebab, type BuildArgs } from '../args.ts';
import { findTsConfig, type BuildContext } from './context.ts';
import { runWebpack } from './run-webpack.ts';
import { writeImportMap } from './import-map.ts';
import { systemConfig, vendorConfig, mainConfig } from './configs.ts';

// 🔧 Build one foil: import map, SystemJS runtime, vendor modules, then main.
export async function buildFoil(args: BuildArgs) {
  const buildDirAbs = join(args.rootPath, args.output);
  const mainTitle =
    args.mainTitle.length < 1 ? parse(args.input).name : args.mainTitle;
  const libraryName = args.name.length < 1 ? kebab(mainTitle) : args.name;
  const tsConfigFile = await findTsConfig(args.rootPath);
  if (!tsConfigFile) {
    throw new Error(
      `Could not find a tsconfig.json in ${args.rootPath} or the current working directory.`
    );
  }
  const ctx: BuildContext = {
    nodeEnvStr: args.production ? 'production' : 'development',
    tsConfigFile,
    cacheDir: join(args.rootPath, 'node_modules', '.cache', 'foil-webpack')
  };

  if (args.inputMap) {
    await writeImportMap(args, libraryName, buildDirAbs);
  }

  if (args.system) {
    await runWebpack('🌄 SystemJS', systemConfig(args, ctx, buildDirAbs), ctx);
  }

  if (args.publicModules.length > 0 && args.vendor) {
    await Promise.all(
      args.publicModules.map((m) => {
        const web = /((server)|(ssr))/.exec(m) === null;
        return runWebpack(
          `📚 ${m} (${web ? 'web' : 'server'})`,
          vendorConfig(args, ctx, buildDirAbs, m),
          ctx
        );
      })
    );
  }

  if (args.input.length > 0) {
    await runWebpack(
      '✨ ' + mainTitle,
      mainConfig(args, ctx, buildDirAbs, libraryName),
      ctx
    );
  }
}
