import chalk from 'chalk';
const { gray, green, cyan, red } = chalk;
import webpack, { Configuration, Compiler } from 'webpack';
import type { BuildContext } from './context.ts';

// 🔧 Run a Webpack compilation with the filesystem cache and log the result.
export function runWebpack(
  title: string,
  config: Configuration,
  ctx: BuildContext
) {
  const buildTitle = title + gray(` (${ctx.nodeEnvStr}) `);
  const compiler: Compiler = webpack({
    ...config,
    cache: {
      type: 'filesystem',
      cacheDirectory: ctx.cacheDir,
      name: `${ctx.nodeEnvStr}-${title.replace(/[^a-z0-9._-]+/gi, '-')}`,
      buildDependencies: { config: [ctx.tsConfigFile] }
    }
  });
  return new Promise((res, rej) => {
    const webpackCallback = (err, stats) => {
      // Closing the compiler flushes the filesystem cache.
      compiler.close((closeErr) => {
        if (err || closeErr) {
          console.error(err || closeErr);
          return rej(err || closeErr);
        }

        if (stats.hasErrors()) {
          let statsJson = stats.toJson();
          console.log(
            '❌' + red(' · Error · ') + buildTitle + ' failed to compile:'
          );
          for (let error of statsJson.errors) {
            console.warn(error.message);
          }
          return rej(new Error(title + ' failed to compile.'));
        }
        console.log(
          '✔️️' +
            green('  · Success · ') +
            buildTitle +
            ' built in ' +
            cyan(+stats.endTime - +stats.startTime + ' ms.')
        );
        return res(stats);
      });
    };
    compiler.run(webpackCallback);
  });
}
