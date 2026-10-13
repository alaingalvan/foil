// Build inputs shared by one-shot argv parsing and server `build` payloads.
export interface BuildArgs {
  name: string;
  mainTitle: string;
  rootPath: string;
  input: string;
  output: string;
  system: boolean;
  inputMap: boolean;
  publicModules: string[];
  vendor: boolean;
  production: boolean;
}

export const kebab = (str: string) =>
  str
    .replace(/\-/, '') //replace all special chars
    .replace(/\s\s+/, ' ') //replace multi-spaces
    .replace(/([a-z])([A-Z])/g, '$1-$2') // get all lowercase letters that are near to uppercase ones
    .replace(/[\s_]+/g, '-') // replace all spaces and low dash
    .toLowerCase(); // convert to lower case

// foil-builder --root-path <path> --system --public-modules <list> --vendor --input <main> --output <dir>
export function parseArgs(argv: string[]): BuildArgs {
  let args = {
    name: '',
    mainTitle: '',
    rootPath: '',
    input: '',
    output: '',
    system: false,
    inputMap: false,
    publicModules: [],
    vendor: false,
    production: false || process?.env['NODE_ENV']?.match(/production/) != null
  };
  for (let i = 0; i < argv.length; i++) {
    let arg = argv[i];
    // Arg matches --args map:
    let found: string = Object.keys(args).reduce(
      (p, c) => ('--' + kebab(c) === arg ? c : p),
      ''
    );
    if (found.length > 0) {
      if (Array.isArray(args[found])) {
        // Look ahead until we find another argument or we're out of arguments.
        let lookForward = true;
        while (lookForward) {
          i++;
          if (i < argv.length) {
            let notFoundForward = !Object.keys(args).reduce(
              (p, c) => p || ('--' + kebab(c) === argv[i] ? true : false),
              false
            );
            if (notFoundForward) {
              args[found].push(argv[i]);
            }
            lookForward = lookForward && notFoundForward && i < argv.length;
            if (!lookForward) i--;
          } else {
            lookForward = false;
          }
        }
      } else if (typeof args[found] === 'string') {
        if (i + 1 < argv.length) {
          args[found] = argv[i + 1];
        }
      } else if (typeof args[found] == 'boolean') {
        args[found] = true;
      }
    }
  }

  return args;
}
