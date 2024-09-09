use crate::builder::BuildMode;
use crate::builder::{get_foil_builder_path, get_foil_folder_path};
use crate::error::Result;
use crate::misc::connect_db;
use crate::misc::{get_db_url, DATABASE_URL};
use crate::query_post::query_post;
use chrono::{DateTime, Utc};
use lexiclean::Lexiclean;
use path_slash::PathBufExt;
use std::env;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::process;
use std::process::Stdio;

fn clean_path_string(p: &std::path::PathBuf) -> String {
    p.clone()
        .lexiclean()
        .to_slash()
        .unwrap()
        .to_string()
        .replace("\\", "/")
}

//=====================================================================================================================
// NPM is somewhat buggy at times, and requires the extension on windows.
#[cfg(windows)]
const FOIL_BACKEND: &'static str = "foil_backend.exe";

#[cfg(not(windows))]
const FOIL_BACKEND: &'static str = "foil_backend";

pub async fn start_server(_build_mode: BuildMode) -> Result<()> {
    // 📚 Configure database...
    let pool = connect_db().await?;
    let cwd = env::current_dir().unwrap_or_default();

    // 📦 Resolve Foil package.json from current working directory, attempt to run server from it:
    match query_post(&pool, "/".to_string()).await {
        Ok(post) => {
            // A few common vars:
            let foil_folder_path = get_foil_folder_path();
            let foil_builder_path = get_foil_builder_path();
            let foil_cache_path = foil_builder_path.join(PathBuf::from("cache"));
            let foil_log_path = foil_folder_path.join(PathBuf::from("log"));

            fs::create_dir_all(&foil_cache_path)?;
            fs::create_dir_all(&foil_log_path)?;

            // 🧻 Create log file for backend errors:
            let dt = Utc::now();
            let naive_utc = dt.naive_utc();
            let offset = dt.offset().clone();
            let date_now = DateTime::<Utc>::from_naive_utc_and_offset(naive_utc, offset);
            let date_now_string = date_now
                .to_string()
                .replace(":", "_")
                .replace(" ", "_")
                .replace(".", "_");
            let backend_log_file = format!("foil-backend-log-{}.txt", &date_now_string);
            let backend_log_file_abs = foil_log_path.clone().join(&backend_log_file);
            let backend_file = File::create(backend_log_file_abs).unwrap();
            let backend_stdio = Stdio::from(backend_file);

            // 🌐 Spawn child processes for the server:
            let foil_database_url = get_db_url();
            let mut backend_server_child = process::Command::new(&FOIL_BACKEND)
                .current_dir(&cwd)
                .stdin(process::Stdio::null())
                .env_remove("args")
                .env(DATABASE_URL, foil_database_url)
                .env("RUST_LOG", env::var("RUST_LOG").unwrap_or_default())
                .stderr(backend_stdio)
                .spawn()
                .unwrap();

            // 🎨 The Foil server-side renderer currently exists as a separate process:
            let frontend_main = post.name.clone();
            let main_path = clean_path_string(&PathBuf::from(&post.output_path).join("main.js"));
            let mut import_map_str =
                format!("        \"{}\": \"file:///{}\",\n", &post.name, main_path);
            for public_module in post.public_modules {
                let mod_path = clean_path_string(
                    &PathBuf::from(&post.output_path).join(public_module.clone() + ".js"),
                );
                import_map_str += &format!(
                    "        \"{}\": \"file:///{}\",\n",
                    &public_module, mod_path
                );
            }
            let server_src = format!(
                include_str!("server-renderer-template.txt"),
                frontend_main,
                import_map_str,
                include_str!("server-renderer.js")
            );

            // Due to the idiosyncrasies of Node.js and TypeScript, we must generate and load a renderer file:
            // Also, due to the way node resolves modules, it must exist next to the frontend.
            // We may want to clear this cache path in the future...

            let server_source_file = frontend_main + "-renderer.generated.mjs";
            let server_source_file_abs = foil_cache_path.clone().join(&server_source_file);
            let server_source_file_abs_str = clean_path_string(&server_source_file_abs);

            let write_result = fs::write(&server_source_file_abs, server_src);
            if write_result.is_err() {
                println!(
            "❌ Failed to write foil renderer.generated.js in current working directory, aborting."
        );
                return Ok(());
            }

            let cwd_node_modules = clean_path_string(&cwd.join("node_modules"));
            let builder_node_modules = clean_path_string(&foil_builder_path.join("node_modules"));
            let node_path_str = builder_node_modules + ";" + &cwd_node_modules;

            let renderer_log_file = format!("foil-renderer-log-{}.txt", &date_now_string);
            let renderer_log_file_abs = foil_log_path.clone().join(&renderer_log_file);
            let renderer_file = File::create(renderer_log_file_abs).unwrap();
            let renderer_stdio = Stdio::from(renderer_file);

            let mut backend_renderer_child = process::Command::new("node")
                .current_dir(&cwd)
                .env("NODE_PATH", node_path_str)
                .args([
                    "--experimental-specifier-resolution=node",
                    "--experimental-modules",
                    "--experimental-import-meta-resolve",
                    "--no-warnings",
                    "--trace-warnings",
                    &server_source_file_abs_str,
                ])
                .stderr(renderer_stdio)
                .spawn()
                .unwrap();

            backend_renderer_child
                .wait()
                .expect("❌ Failed to run Foil Renderer...");
            backend_server_child
                .wait()
                .expect("❌ Failed to run Foil Backend...");
        }
        Err(e) => {
            println!("Failed to connect to database when starting server.\n{}", e);
        },
    };
    Ok(())
}
