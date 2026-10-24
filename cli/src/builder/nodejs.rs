use chrono::{DateTime, Utc};

use super::imports::resolve_imports;
use super::metadata::FoilMetadataStatus;
use super::server::{BuilderServer, PendingBuild};
use super::resolver::Foil;
use super::static_assets::FoilFile;
use crate::{BuildMode, Result};
use std::path::PathBuf;
use std::process::Command;

//=====================================================================================================================
// NPM is somewhat buggy at times, and requires the extension on windows.
#[cfg(windows)]
const NPM: &'static str = "npm.cmd";

#[cfg(not(windows))]
const NPM: &'static str = "npm";

//=====================================================================================================================
/// 🔎 Find all imports of a given main JS/TS file's dependency tree.
pub fn find_all_imports(main: String, root_path: &PathBuf) -> Vec<FoilFile> {
    let mut foil_files: Vec<FoilFile> = vec![];
    for import_path in resolve_imports(&root_path.join(&main)) {
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
/// `npm ci` deletes and reinstalls `node_modules`, so only run it when the install is missing or older than the lock file.
fn needs_install(root_path: &PathBuf) -> bool {
    let modified = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let Some(installed) = modified(root_path.join("node_modules").join(".package-lock.json"))
    else {
        return true;
    };
    ["package-lock.json", "package.json"]
        .iter()
        .filter_map(|name| modified(root_path.join(name)))
        .any(|source| source > installed)
}

//=====================================================================================================================
/// 🍱 Compile a given foil module with Webpack.
pub fn compile_foil_main(
    mode: BuildMode,
    resolved_foil: &Foil,
    foil_changed: FoilMetadataStatus,
    server: &BuilderServer,
) -> Result<PendingBuild> {
    // ⤵️ Install all dependencies for this project if they don't exist.
    let _ci = if !needs_install(&resolved_foil.root_path) {
        Ok(())
    } else {
        Command::new(NPM)
            .current_dir(&resolved_foil.root_path)
            .arg("ci")
            .arg("--include=dev")
            .output()
            .map(|_| ())
    };
    let _ci = match _ci {
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

    let frontend = resolved_foil.frontend;
    let payload = serde_json::json!({
        "name": resolved_foil.name,
        "mainTitle": resolved_foil.title,
        "rootPath": root_path_str,
        "output": output_path_str,
        "input": if foil_changed.files_changed { resolved_foil.main.as_str() } else { "" },
        "system": frontend && foil_changed.runtime_changed,
        "inputMap": frontend && foil_changed.public_modules_changed,
        "vendor": frontend && foil_changed.public_modules_changed,
        "publicModules": resolved_foil.public_modules,
        "production": matches!(mode, BuildMode::Release),
    });
    server.build(&resolved_foil.title, payload)
}
