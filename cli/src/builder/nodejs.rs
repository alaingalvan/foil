use chrono::{DateTime, Utc};

use super::get_foil_builder_path;
use super::metadata::FoilMetadataStatus;
use super::resolver::Foil;
use super::static_assets::FoilFile;
use crate::{BuildMode, Result};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

//=====================================================================================================================
// NPM is somewhat buggy at times, and requires the extension on windows.
#[cfg(windows)]
const NPM: &'static str = "npm.cmd";

#[cfg(not(windows))]
const NPM: &'static str = "npm";

//=====================================================================================================================
/// 🔎 Find all imports of a given main JS/TS file's dependency tree.
pub fn find_all_imports(main: String, root_path: &PathBuf) -> Vec<FoilFile> {
    // The foil builder exists next to the current executable:
    let foil_builder_path = get_foil_builder_path();

    // Run the import resolution script. We assume the builder's already configured with its node_modules.
    let root_path_string = root_path
        .to_str()
        .unwrap_or("/")
        .to_string()
        .replace("\\", "/");
    let main_abs_str = root_path
        .join(PathBuf::from(&main))
        .to_str()
        .unwrap_or("/")
        .to_string()
        .replace("\\", "/");
    let find = Command::new("node")
        .current_dir(&foil_builder_path)
        .args(["dist/resolve-imports.js", &root_path_string, &main_abs_str])
        .output()
        .unwrap();
    let out_string = String::from_utf8(find.stdout).unwrap_or("[]".to_string());
    // TODO: should this just be string paths, and we figure out the modified date?
    let data: Vec<String> = match serde_json::from_str(&out_string) {
        Ok(v) => v,
        Err(_er) => {
            vec![]
        }
    };

    let mut foil_files: Vec<FoilFile> = vec![];
    for import in data {
        let import_path = PathBuf::from(import);
        let clean_path = import_path
            .to_str()
            .unwrap_or("/")
            .to_string()
            .replace("\\", "/");

        let modified_date = match import_path.metadata() {
            Ok(m) => {
                DateTime::<Utc>::from(m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            }
            _ => DateTime::<Utc>::MIN_UTC,
        };
        let foil_file = FoilFile {
            path: clean_path,
            modified_date,
        };
        foil_files.push(foil_file);
    }

    // Include <root_path>/package.json:
    {
        let package_path = root_path.join(PathBuf::from("package.json"));
        let clean_path = package_path
            .to_str()
            .unwrap_or("/")
            .to_string()
            .replace("\\", "/");

        let modified_date = match package_path.metadata() {
            Ok(m) => {
                DateTime::<Utc>::from(m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            }
            _ => DateTime::<Utc>::MIN_UTC,
        };
        let foil_file = FoilFile {
            path: clean_path,
            modified_date,
        };
        foil_files.push(foil_file);
    }
    // Sort by path and modified date:
    foil_files.sort_by(|a, b| a.path.cmp(&b.path));
    foil_files.sort_by_key(|k| k.modified_date.timestamp());

    foil_files
}

//=====================================================================================================================
/// 🍱 Compile a given foil module with Webpack.
pub fn compile_foil_main(
    mode: BuildMode,
    resolved_foil: &Foil,
    foil_changed: FoilMetadataStatus,
) -> Result<Child> {
    // ⤵️ Install all dependencies for this project if they don't exist.
    let _ci = match Command::new(NPM)
        .current_dir(&resolved_foil.root_path)
        .arg("ci")
        .arg("--include=dev")
        .output()
    {
        Ok(_) => (),
        Err(_e) => {
            // There may not be a package-lock.json file, add one:
            println!("Failed to run `npm ci`, running `npm i`:");
            let _i = match Command::new(NPM)
                .current_dir(&resolved_foil.root_path)
                .arg("i")
                .arg("--include=dev")
                .output()
            {
                Ok(_) => (),
                e => {
                    println!("Failed to run `npm i`, error:\n{:?}", e);
                }
            };
        }
    };

    // 🔨 Build foil project using node.js and webpack.
    // This builds the output, and optionally the SystemJS runtime, import map, and vendor modules.
    // The foil builder exists next to the current executable:
    let foil_builder_path = get_foil_builder_path();
    let root_path_str = resolved_foil
        .root_path
        .to_str()
        .unwrap_or("/")
        .to_string()
        .replace("\\", "/");

    let output_path_str = resolved_foil
        .output_path
        .to_str()
        .unwrap_or("/")
        .to_string()
        .replace("\\", "/")
        .replace(&root_path_str, "");

    let mut compile = Command::new("node");
    compile.current_dir(&foil_builder_path).args([
        "--experimental-specifier-resolution=node",
        "--experimental-modules",
        "--experimental-import-meta-resolve",
        "--no-warnings",
        "dist/foil-builder.js",
        "--name",
        &resolved_foil.name,
        "--main-title",
        &resolved_foil.title,
        "--root-path",
        &root_path_str,
        "--output",
        &output_path_str,
    ]);

    if foil_changed.files_changed {
        compile.arg("--input");
        compile.arg(&resolved_foil.main);
    }
    if resolved_foil.frontend {
        if foil_changed.runtime_changed {
            compile.arg("--system");
        }
        // Output input map
        if foil_changed.public_modules_changed {
            compile.arg("--input-map");
            compile.arg("--vendor");
        }
    }
    match mode {
        BuildMode::Release => {
            compile.arg("--production");
            compile.env("NODE_ENV", "production");
        }
        BuildMode::Development => {
            compile.env("NODE_ENV", "development");
        }
    };

    compile.arg("--public-modules");
    compile.args(&resolved_foil.public_modules);

    let output = compile
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .spawn()
        .unwrap();

    Ok(output)
}
