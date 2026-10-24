use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

//=====================================================================================================================
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Ts,
    Js,
    Mdx,
    Other,
}

fn kind_of(path: &Path) -> Kind {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "ts" | "tsx" | "mts" | "cts" => Kind::Ts,
        "js" | "jsx" | "mjs" | "cjs" => Kind::Js,
        "mdx" => Kind::Mdx,
        _ => Kind::Other,
    }
}

/// Extensions tried, in order, for an extension-less specifier.
fn probe_extensions(importer: Kind) -> &'static [&'static str] {
    match importer {
        Kind::Ts => &[".ts", ".tsx", ".d.ts"],
        Kind::Js => &[".js", ".jsx", ".mjs", ".cjs", ".json"],
        Kind::Mdx => &[".mdx", ".tsx", ".ts", ".jsx", ".js"],
        Kind::Other => &[],
    }
}

//=====================================================================================================================
/// 🔎 Every local file the entry file depends on, including the entry itself and imported assets.
/// Only relative specifiers are followed. Targets inside `node_modules` are ignored.
pub fn resolve_imports(main: &Path) -> Vec<PathBuf> {
    let main = normalize(main);
    if kind_of(&main) == Kind::Other || !main.is_file() {
        return vec![];
    }
    let mut seen: HashSet<PathBuf> = HashSet::new();
    visit(&main, &mut seen);
    let mut files: Vec<PathBuf> = seen.into_iter().collect();
    files.sort();
    files
}

fn visit(file: &Path, seen: &mut HashSet<PathBuf>) {
    if !seen.insert(file.to_path_buf()) {
        return;
    }
    let importer = kind_of(file);
    // Assets are leaves, their contents are never parsed.
    if importer == Kind::Other {
        return;
    }
    let Ok(bytes) = fs::read(file) else {
        return;
    };
    let source = String::from_utf8_lossy(&bytes);
    let specifiers = if importer == Kind::Mdx {
        mdx_specifiers(&source)
    } else {
        scan_specifiers(&source)
    };
    let dir = file.parent().unwrap_or(Path::new(""));
    for specifier in specifiers {
        if let Some(target) = resolve_specifier(dir, &specifier, importer) {
            visit(&target, seen);
        }
    }
}

//=====================================================================================================================
fn resolve_specifier(dir: &Path, specifier: &str, importer: Kind) -> Option<PathBuf> {
    // Strip webpack loader prefixes (`!!raw-loader!./a.glsl`) and `?query` / `#hash` suffixes.
    let specifier = specifier.rsplit('!').next().unwrap_or(specifier);
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return None;
    }
    let specifier = specifier.split(['?', '#']).next().unwrap_or(specifier);
    let base = normalize(&dir.join(specifier));
    let found = probe(&base, importer)?;
    if found.components().any(|c| c.as_os_str() == "node_modules") {
        return None;
    }
    Some(found)
}

fn probe(base: &Path, importer: Kind) -> Option<PathBuf> {
    let append = |ext: &str| -> PathBuf {
        let mut s: OsString = base.as_os_str().to_owned();
        s.push(ext);
        PathBuf::from(s)
    };

    if base.extension().is_some() && base.is_file() {
        return Some(base.to_path_buf());
    }
    // TypeScript's ESM convention: `./x.js` names `x.ts`.
    if matches!(importer, Kind::Ts | Kind::Mdx) && kind_of(base) == Kind::Js {
        for ext in ["ts", "tsx"] {
            let candidate = base.with_extension(ext);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let extensions = probe_extensions(importer);
    for ext in extensions {
        let candidate = append(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if base.is_file() {
        return Some(base.to_path_buf());
    }
    if base.is_dir() {
        for ext in extensions {
            let candidate = base.join(format!("index{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Lexically resolve `.` and `..`, without touching the file system.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

//=====================================================================================================================
/// MDX imports live in ESM blocks: a line starting with `import`/`export` runs until the next blank line.
/// Fenced code blocks are skipped.
fn mdx_specifiers(source: &str) -> Vec<String> {
    let mut specifiers = vec![];
    let mut block = String::new();
    let mut fence: Option<&str> = None;

    let flush = |block: &mut String, specifiers: &mut Vec<String>| {
        if !block.is_empty() {
            specifiers.extend(scan_specifiers(block));
            block.clear();
        }
    };

    for line in source.lines() {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (Some(open), Some(m)) if open == m => {
                fence = None;
                continue;
            }
            (None, Some(m)) => {
                flush(&mut block, &mut specifiers);
                fence = Some(m);
                continue;
            }
            (Some(_), _) => continue,
            _ => {}
        }

        if line.trim().is_empty() {
            flush(&mut block, &mut specifiers);
        } else if !block.is_empty() || is_esm_start(line) {
            block.push_str(line);
            block.push('\n');
        }
    }
    flush(&mut block, &mut specifiers);
    specifiers
}

fn is_esm_start(line: &str) -> bool {
    ["import", "export"].iter().any(|kw| {
        line.strip_prefix(kw).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || matches!(c, '{' | '*' | '"' | '\'' | '('))
        })
    })
}

//=====================================================================================================================
enum Tok<'a> {
    Ident(&'a str),
    Str(&'a str),
    Punct(u8),
}

/// A token-level scan, not a parser. Comments and template literal contents are skipped, and an unterminated quote
/// (such as an apostrophe in JSX text) is dropped rather than swallowing the rest of the file.
fn tokenize(src: &str) -> Vec<Tok<'_>> {
    let b = src.as_bytes();
    let mut tokens = vec![];
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            _ if c.is_ascii_whitespace() => i += 1,
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i += 2;
            }
            b'"' | b'\'' => {
                let mut j = i + 1;
                loop {
                    if j >= b.len() || b[j] == b'\n' {
                        i += 1;
                        break;
                    }
                    if b[j] == b'\\' {
                        j += 2;
                    } else if b[j] == c {
                        tokens.push(Tok::Str(&src[i + 1..j]));
                        i = j + 1;
                        break;
                    } else {
                        j += 1;
                    }
                }
            }
            b'`' => i = skip_template(b, i + 1),
            _ if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$') || c >= 0x80 => {
                let start = i;
                while i < b.len()
                    && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'_' | b'$') || b[i] >= 0x80)
                {
                    i += 1;
                }
                tokens.push(Tok::Ident(&src[start..i]));
            }
            _ => {
                tokens.push(Tok::Punct(c));
                i += 1;
            }
        }
    }
    tokens
}

fn skip_template(b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'`' => return i + 1,
            b'$' if b.get(i + 1) == Some(&b'{') => {
                i += 2;
                let mut depth = 1;
                while i < b.len() && depth > 0 {
                    match b[i] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        b'`' => {
                            i = skip_template(b, i + 1);
                            continue;
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    b.len()
}

/// Specifiers from `import`, `export ... from`, `require()` and `import()`, including type-only forms.
fn scan_specifiers(src: &str) -> Vec<String> {
    let tokens = tokenize(src);
    let mut out = vec![];
    let after_dot = |idx: usize| idx > 0 && matches!(tokens[idx - 1], Tok::Punct(b'.'));

    for (idx, token) in tokens.iter().enumerate() {
        let Tok::Ident(word) = token else { continue };
        if after_dot(idx) {
            continue;
        }
        match *word {
            "import" => match tokens.get(idx + 1) {
                Some(Tok::Str(s)) => out.push(s.to_string()),
                Some(Tok::Punct(b'(')) => {
                    if let Some(Tok::Str(s)) = tokens.get(idx + 2) {
                        out.push(s.to_string());
                    }
                }
                _ => scan_from_clause(&tokens, idx + 1, &mut out),
            },
            "export" => scan_from_clause(&tokens, idx + 1, &mut out),
            "require" => {
                if let (Some(Tok::Punct(b'(')), Some(Tok::Str(s))) =
                    (tokens.get(idx + 1), tokens.get(idx + 2))
                {
                    out.push(s.to_string());
                }
            }
            _ => {}
        }
    }
    out
}

/// Walk binding syntax (`a, { b as c }, * as d`) until `from '<specifier>'`, giving up on anything else.
fn scan_from_clause(tokens: &[Tok], start: usize, out: &mut Vec<String>) {
    let mut k = start;
    while let Some(token) = tokens.get(k) {
        match token {
            Tok::Ident("from") => {
                if let Some(Tok::Str(s)) = tokens.get(k + 1) {
                    out.push(s.to_string());
                    return;
                }
            }
            Tok::Ident(_) | Tok::Punct(b'{' | b'}' | b',' | b'*') => {}
            _ => return,
        }
        k += 1;
    }
}

//=====================================================================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("test-assets")
            .join("imports")
    }

    /// Resolved files relative to the fixtures folder, with `/` separators, sorted.
    fn resolved(main: &str) -> Vec<String> {
        let root = fixtures();
        let mut names: Vec<String> = resolve_imports(&root.join(main))
            .iter()
            .map(|p| {
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        names
    }

    fn sorted(items: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = items.iter().map(|s| s.to_string()).collect();
        v.sort();
        v
    }

    #[test]
    fn tsx_entry_follows_code_assets_and_nested_mdx() {
        assert_eq!(
            resolved("library/main.tsx"),
            sorted(&[
                "library/main.tsx",
                "library/description.mdx",
                "library/components/callout.tsx",
                "library/components/hero/index.tsx",
                "library/components/hero/hero.css",
                "library/styles/base.scss",
                "library/styles/theme.css",
                "library/shaders/main.glsl",
                "library/shaders/compute.wgsl",
                "library/lazy.ts",
                "library/cjs-helper.js",
                "library/reexport.ts",
                "shared/util.ts",
            ])
        );
    }

    #[test]
    fn mdx_entry_resolves_exactly_and_reads_multiline_blocks() {
        assert_eq!(
            resolved("blog/index.mdx"),
            sorted(&[
                "blog/index.mdx",
                "blog/citations.ts",
                "blog/widgets/chart.tsx",
                "blog/widgets/math.ts",
                "blog/widgets/multi.ts",
            ])
        );
    }

    #[test]
    fn mdx_ignores_fenced_code_and_prose() {
        let found = resolved("blog/index.mdx");
        assert!(!found.contains(&"blog/in-fence.ts".to_string()));
        assert!(!found.contains(&"blog/in-prose.ts".to_string()));
    }

    #[test]
    fn same_named_file_elsewhere_is_not_matched() {
        assert!(!resolved("blog/index.mdx").contains(&"blog/nested/citations.ts".to_string()));
    }

    #[test]
    fn bare_missing_and_commented_are_ignored() {
        let found = resolved("library/main.tsx");
        for ignored in ["library/commented.ts", "library/missing.ts"] {
            assert!(!found.contains(&ignored.to_string()), "{ignored}");
        }
    }

    // Built in a temp dir because `node_modules` is gitignored.
    #[test]
    fn node_modules_targets_are_ignored() {
        let dir = std::env::temp_dir().join(format!("foil-imports-{}", std::process::id()));
        let pkg = dir.join("node_modules").join("pkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("index.js"), "").unwrap();
        let main = dir.join("main.ts");
        fs::write(&main, "import './node_modules/pkg/index.js';").unwrap();
        assert_eq!(resolve_imports(&main), vec![normalize(&main)]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cycles_terminate() {
        assert_eq!(resolved("cycle/a.ts"), sorted(&["cycle/a.ts", "cycle/b.ts"]));
    }

    #[test]
    fn js_does_not_resolve_ts_siblings() {
        assert_eq!(
            resolved("js-only/main.js"),
            sorted(&["js-only/main.js", "js-only/helper.js", "js-only/data.json"])
        );
    }

    #[test]
    fn type_definitions_and_js_extension_alias_resolve_from_ts() {
        assert_eq!(
            resolved("dts/main.ts"),
            sorted(&["dts/main.ts", "dts/types.d.ts", "dts/impl.ts"])
        );
    }

    #[test]
    fn missing_or_unsupported_entry_is_empty() {
        assert!(resolved("library/nope.tsx").is_empty());
        assert!(resolved("library/shaders/main.glsl").is_empty());
    }

    #[test]
    fn scanner_covers_import_forms() {
        let src = r#"
            import a from './a';
            import type { B } from "./b";
            import * as c from './c'
            import './d';
            export * from './e';
            export { f } from './f';
            export type { G } from './g';
            const h = require('./h');
            const i = import('./i');
            import j = require('./j');
            // import './comment';
            /* import './block'; */
            const t = `import './template'`;
            export const k = 1;
            const meta = import.meta.url;
        "#;
        assert_eq!(
            scan_specifiers(src),
            ["./a", "./b", "./c", "./d", "./e", "./f", "./g", "./h", "./i", "./j"]
        );
    }

    #[test]
    fn scanner_survives_jsx_apostrophes() {
        let src = "import a from './a';\nconst x = <p>don't stop</p>;\nimport b from './b';";
        assert_eq!(scan_specifiers(src), ["./a", "./b"]);
    }

    #[test]
    fn loader_prefix_and_query_are_stripped() {
        let dir = fixtures().join("library");
        let target = resolve_specifier(&dir, "!!raw-loader!./shaders/main.glsl?raw", Kind::Ts);
        assert_eq!(target, Some(normalize(&dir.join("shaders/main.glsl"))));
    }
}
