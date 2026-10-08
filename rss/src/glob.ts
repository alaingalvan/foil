/** Turn a Foil permalink glob (`/blog/*`) into the regex the posts table expects. */
export function globToRegex(glob: string): string {
  let pattern = "^";
  for (const char of glob) {
    if ("/$^+.()=!|,{}[]".includes(char)) {
      pattern += "\\" + char;
    } else if (char === "*") {
      pattern += ".*";
    } else {
      pattern += char;
    }
  }
  return pattern + "$";
}
