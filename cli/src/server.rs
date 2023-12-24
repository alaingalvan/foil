use crate::builder::read_foil_package;
use crate::builder::BuildMode;
use crate::misc::{get_db_url, DATABASE_URL};
use std::env;
use std::fs;
use std::process;

//=====================================================================================================================
// NPM is somewhat buggy at times, and requires the extension on windows.
#[cfg(windows)]
const FOIL_BACKEND: &'static str = "foil_backend.exe";

#[cfg(not(windows))]
const FOIL_BACKEND: &'static str = "foil_backend";

pub async fn start_server(_build_mode: BuildMode) {
    // 📦 Resolve Foil package.json from current working directory, attempt to run server from it:
    let cwd = env::current_dir().unwrap_or_default();
    let cwd_package = cwd.join("package.json");
    let cwd_foil_package_res = read_foil_package(&cwd_package);
    if cwd_foil_package_res.is_err() {
        println!("❌ Failed to find foil package.json in current working directory, aborting.");
        return;
    }
    let cwd_foil_package = cwd_foil_package_res.unwrap();
    if cwd_foil_package.foil.html.is_empty() {
        println!("❌ Foil package.json's 'foil' object must have a valid 'html' member pointing to a JavaScript/TypeScript file with a React element default export.");
        return;
    }

    // ⚽ A foil website needs a default path to public files, generally a `static` or `assets` folder:
    let assets_path = if cwd_foil_package.foil.assets.is_empty() {
        cwd.join("assets")
    } else {
        let assets_sanitized = cwd_foil_package.foil.assets[0].replace("*", "");
        cwd.join(assets_sanitized)
    }
    .to_str()
    .unwrap_or_default()
    .replace("\\", "/");

    // 🌐 Spawn child processes for the server:
    let foil_database_url = get_db_url();
    let mut backend_server_child = process::Command::new(&FOIL_BACKEND)
        .current_dir(&cwd)
        .stdin(process::Stdio::null())
        .env_remove("args")
        .env(DATABASE_URL, foil_database_url)
        .arg(&assets_path)
        .spawn()
        .unwrap();

    // 🎨 The Foil server-side renderer currently exists as a separate process here,
    // we may eventually move this to the server itself where it pipes requests to a child process...
    let mut server_source =
        "// ⚠️ Warning: This file was generated by Foil and doesn't need to be touched.\n"
            .to_string();
    server_source += "import Default from \"./";
    server_source += &cwd_foil_package.foil.html;
    server_source += "\";\n";
    server_source += include_str!("server-renderer.tsx");

    // Due to the idiosyncrasies of Node.js and TypeScript, we must generate and load a renderer file:
    let server_source_file = "foil-renderer.generated.tsx";
    let write_result = fs::write(cwd.join(server_source_file), server_source);
    if write_result.is_err() {
        println!(
            "❌ Failed to write foil-renderer.generated.tsx in current working directory, aborting."
        );
        return;
    }

    let mut backend_renderer_child = process::Command::new("node")
        .current_dir(&cwd)
        .args([
            "--experimental-specifier-resolution=node",
            "--experimental-modules",
            "--experimental-import-meta-resolve",
            "--no-warnings",
            "--loader",
            "ts-node/esm",
            &server_source_file,
        ])
        .spawn()
        .unwrap();

    backend_renderer_child
        .wait()
        .expect("❌ Failed to run Foil Renderer...");
    backend_server_child
        .wait()
        .expect("❌ Failed to run Foil Backend...");
}
