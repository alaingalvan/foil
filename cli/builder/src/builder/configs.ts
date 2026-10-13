import { join, resolve } from 'path';
import webpack, { Configuration } from 'webpack';
const { DefinePlugin } = webpack;
import MiniCssExtractPlugin from 'mini-css-extract-plugin';
import CssMinimizerPlugin from 'css-minimizer-webpack-plugin';
import rehypeKatex from 'rehype-katex';
import rehypeSlug from 'rehype-slug';
import remarkCode from '../misc/remark-code.ts';
import remarkMath from 'remark-math';
import remarkGfm from 'remark-gfm';
import type { BuildArgs } from '../args.ts';
import type { BuildContext } from './context.ts';

// Webpack configurations, one per target.
export function systemConfig(
  args: BuildArgs,
  ctx: BuildContext,
  buildDirAbs: string
): Configuration {
  let suffix = args.production ? '.min' : '';
  return {
    mode: ctx.nodeEnvStr,
    entry: {
      system: [
        'systemjs/dist/system' + suffix + '.js',
        'systemjs/dist/extras/named-register' + suffix + '.js',
        'systemjs/dist/extras/dynamic-import-maps' + suffix + '.js'
      ]
    },
    output: {
      path: buildDirAbs,
      filename: '[name].js'
    },
    devtool: args.production ? false : 'inline-source-map',
    optimization: {
      minimize: args.production
    }
  };
}

export function vendorConfig(
  args: BuildArgs,
  ctx: BuildContext,
  buildDirAbs: string,
  m: string
): Configuration {
  let web = /((server)|(ssr))/.exec(m) === null;
  let externals = args.publicModules.reduce(
    (prev, cur) => (m != cur ? { ...prev, [cur]: cur } : prev),
    {}
  );
  return {
    mode: ctx.nodeEnvStr,
    target: web ? 'web' : 'node',
    entry: m,
    output: {
      path: buildDirAbs,
      filename: m + '.js',
      libraryTarget: 'system',
      library: {
        type: 'system',
        name: m
      }
    },
    resolve: {
      extensions: ['.ts', '.tsx', '.js'],
      modules: [
        resolve(args.rootPath, 'src'),
        join(args.rootPath, 'node_modules'),
        join(process.cwd(), 'node_modules'),
        'node_modules'
      ],
      fallback: {
        fs: false,
        tls: false,
        net: false,
        path: false,
        zlib: false,
        http: false,
        https: false,
        stream: false,
        crypto: false
      }
    },

    module: {
      rules: [
        // React imports can behave strangely when mixing commonJS with ES modules
        // with default imports. This explicitly defines a default import as a wildcard.
        {
          test: /\.(j|t)sx?$/,
          loader: 'string-replace-loader',
          options: {
            multiple: [
              {
                search: /import\s+React\s+from\s+('|")react('|")/i,
                replace: "import * as React from 'react'"
              },
              {
                search:
                  /(import\s+React,\s+){(\s*.*\s*)}\s*(from\s+('|")react('|"))/,
                replace: (_match, _p1, p2) =>
                  `import * as React from 'react';const {${p2}} = React`,
                flags: 'g'
              }
            ]
          }
        }
      ]
    },
    externalsType: 'system',
    externals,
    externalsPresets: {
      web
    },
    devtool: args.production ? false : 'inline-source-map',
    plugins: [
      new DefinePlugin({
        'process.env': {
          NODE_ENV: JSON.stringify(ctx.nodeEnvStr)
        }
      })
    ],
    optimization: {
      minimize: args.production ? true : false
    }
  };
}

export function mainConfig(
  args: BuildArgs,
  ctx: BuildContext,
  buildDirAbs: string,
  libraryName: string
): Configuration {
  const tsLoaderOptions = {
    transpileOnly: true,
    configFile: ctx.tsConfigFile,
    onlyCompileBundledFiles: true,
    compilerOptions: {
      sourceMap: !args.production,
      rootDir: args.rootPath,
      module: 'ESNext',
      moduleResolution: 'Bundler'
    }
  };
  return {
    mode: ctx.nodeEnvStr,
    context: args.rootPath,
    entry: {
      main: './' + args.input
    },
    output: {
      path: buildDirAbs,
      filename: '[name].js',
      libraryTarget: 'system',
      library: { type: 'system', name: libraryName }
    },
    externalsPresets: {
      web: true
    },
    externalsType: 'system',
    externals: args.publicModules,
    resolve: {
      extensions: ['.ts', '.tsx', '.js'],
      modules: [
        resolve(args.rootPath, 'src'),
        join(args.rootPath, 'node_modules'),
        join(process.cwd(), 'node_modules'),
        'node_modules'
      ],
      fallback: {
        fs: false,
        tls: false,
        net: false,
        path: false,
        zlib: false,
        http: false,
        https: false,
        stream: false,
        crypto: false,
        buffer: false
      }
    },
    module: {
      rules: [
        {
          test: /\.tsx?$/,
          loader: 'ts-loader',
          options: tsLoaderOptions
        },
        {
          test: /\.s?css$/,
          use: [
            {
              loader: MiniCssExtractPlugin.loader
            },
            {
              loader: 'css-loader'
            },
            {
              loader: 'postcss-loader',
              options: {
                postcssOptions: {
                  plugins: [
                    'postcss-nested',
                    'postcss-advanced-variables',
                    'postcss-preset-env',
                    'autoprefixer'
                  ]
                }
              }
            }
          ]
        },
        {
          test: [
            /\.(woff|ttf|eot|svg)(\?v=[a-z0-9]\.[a-z0-9]\.[a-z0-9])?$/,
            /\.(ttf|eot|svg)(\?v=[0-9]\.[0-9]\.[0-9])?$/
          ],
          type: 'asset/resource'
        },
        {
          test: /\.(wgsl|glsl)$/,
          type: 'asset/source'
        },
        {
          test: /\.wasm$/,

          loader: 'file-loader',
          options: {
            name: '[name].[ext]'
          }
        },
        {
          test: /\.mdx$/,
          use: [
            {
              loader: 'ts-loader',
              options: tsLoaderOptions
            },
            {
              loader: '@mdx-js/loader',
              options: {
                jsx: false,
                providerImportSource: '@mdx-js/react',
                remarkPlugins: [remarkMath, remarkCode, remarkGfm],
                rehypePlugins: [rehypeKatex, rehypeSlug]
              }
            }
          ]
        }
      ]
    },
    resolveLoader: {
      modules: ['node_modules', join(process.cwd(), 'node_modules')],
      extensions: ['.js', '.json'],
      mainFields: ['loader', 'main']
    },
    plugins: [
      new DefinePlugin({
        'process.env': {
          NODE_ENV: JSON.stringify(ctx.nodeEnvStr)
        }
      }),
      new MiniCssExtractPlugin({
        filename: '[name].css'
      }) as any
    ],
    devtool: args.production ? false : 'inline-source-map',
    optimization: {
      minimize: args.production ? true : false,
      minimizer: [
        // For webpack@5 you can use the `...` syntax to extend existing minimizers (i.e. `terser-webpack-plugin`), uncomment the next line
        `...`,
        new CssMinimizerPlugin()
      ]
    }
  };
}
