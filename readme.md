<div align="center">

# <a href="https://alain.xyz/blog"><img alt="Foil" src="docs/foil-logo.svg" /></a>

> ✨ Build powerful and flexible portfolios and blogs. ✨

[![License][license-img]][license-url]
[![Unit Tests][travis-img]][travis-url]
[![Coverage Tests][codecov-img]][codecov-url]
[![Dependency Status][david-img]][david-url]
[![devDependency Status][david-dev-img]][david-dev-url]



</div>

Whether you're a *writer, artist, musician, engineer, or all of the above*, this tool makes it easy and fast to showcase a variety of content.

## Features

- 🐙 **Git Powered** with a daemon tool to handle continuous deployment from your git repo, let git be your CMS!

- 🕹️ **Everything is a JavaScript module**, from blog posts to books, music albums, or even custom mini-applications like games or tools. Use JavaScript Modules for it all, and have it all automatically combine and transpile together for your post.

- 🏙️ **A simple and extendable API** for building truly custom portfolios. Define your own data schemas or use our recommended setups for different portfolio types.

- ⚔️ **State of the Art technologies**, [TypeScript](https://www.typescriptlang.org/), [React](https://reactjs.org/), [Webpack](https://webpack.js.org/), [PostCSS](https://postcss.org/), and more. Write views in React, use 3D renderers like Marmoset Viewer, even render academic files written in Markdown + LaTeX, you'll find it all supported here.

Read about some of the *opinions* that guided its design over [here](docs/opinions.md).

## Ecosystem

- 💻 `foil-cli` - A command line interface to help perform tasks to index a foil portfolio, from compiling packages with Webpack to cleaning the database.

## How it Works

### Foil Packages

Every Foil post starts with a [`package.json` file](https://docs.npmjs.com/files/package.json), just like any other Node module, but with the addition of the `foil` object that stores data not defined by [`package.json` specification](https://docs.npmjs.com/files/package.json):

```json
{
  "description": "A cross platform system abstraction library written in C++ for managing windows and performing OS tasks.",
  "main": "main.tsx",
  "keywords": [
    "library",
    "libraries",
    "github",
    "cpp"
  ],
  "foil": {
    "title": "CrossWindow",
    "permalink": "libraries/crosswindow",
    "datePublished": "2018-09-16"
  }
}
```

### File Transformers

Your Foil post's `package.json` points to an entry file, be it JavaScript, TypeScript, Markdown, or a custom file format you want to support.

**Transformers** use a **`test`** object to compare with the current post, and if there's a match, executes a **`transform`** which returns a modified version of a Foil post. For example, here's a transformer for [academically flavored markdown](https://github.com/hyperfuse/markademic):

```ts
import markademic from 'markademic';
import { join } from 'path';
import { readFileSync } from 'fs';

export let md = {
  // 💉 a test object that's used to compare with the `package.json` file.
  test: { file: /\.md$/ },

  // 🚒 the function that takes in the package data and lets you modify it.
  transform: async post => {
    let config = {
      input: readFileSync(post.file).toString(),
      rerouteLinks: (link) => join(post.permalink, link)
    };

    let data = "";

    try {
      data = markademic(config);
    }
    catch (e) {
      console.error('Markademic', e.message);
    }

    return {
      ...post,
      data
    }
  }
}
```

## Licencing

All source code is available with an MIT license, feel free to take bits and pieces and use them in your own projects. I would love to hear how you found things useful, feel free to contact me on Twitter <a href="https://twitter.com/Alainxyz">@alainxyz</a> and let me know.

[cover-img]: docs/assets/logo.png
[cover-url]: https://alain.xyz/libraries/foil
[license-img]: http://img.shields.io/:license-mit-blue.svg?style=flat-square
[license-url]: https://opensource.org/licenses/MIT
[david-url]: https://david-dm.org/alaingalvan/foil?path=packages/foil
[david-img]: https://david-dm.org/alaingalvan/foil.svg?style=flat-square
[david-dev-url]: https://david-dm.org/alaingalvan/foil?path=packages/foil#info=devDependencies
[david-dev-img]: https://david-dm.org/alaingalvan/foil/dev-status.svg?style=flat-square
[travis-img]: https://img.shields.io/travis/alaingalvan/foil.svg?style=flat-square
[travis-url]:https://travis-ci.org/alaingalvan/foil
[codecov-img]:https://img.shields.io/codecov/c/github/alaingalvan/foil.svg?style=flat-square
[codecov-url]: https://codecov.io/gh/alaingalvan/foil
[npm-img]: https://img.shields.io/npm/v/foil.svg?style=flat-square
[npm-url]: http://npm.im/foil
[npm-download-img]: https://img.shields.io/npm/dm/foil.svg?style=flat-square
