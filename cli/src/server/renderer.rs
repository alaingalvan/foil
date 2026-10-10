use super::logging::{clean_path_string, tee_to_console_and_log};
use crate::builder::BuildMode;
use crate::error::Result;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::process::{self, Child, Stdio};

const TRACE_WARNINGS: &str = "FOIL_TRACE_WARNINGS";

pub(super) async fn spawn_renderer(
    post: &crate::query_post::SQLPost,
    cwd: &PathBuf,
    foil_builder_path: &PathBuf,
    foil_cache_path: &PathBuf,
    foil_log_path: &PathBuf,
    build_mode: &BuildMode,
) -> Result<Option<Child>> {
    // 🎨 The Foil server-side renderer currently exists as a separate process:
    let frontend_main = post.name.clone();
    let main_path = clean_path_string(&PathBuf::from(&post.output_path).join("main.js"));
    let mut import_map_str = format!("        \"{}\": \"file:///{}\",\n", &post.name, main_path);
    for public_module in &post.public_modules {
        let mod_path = clean_path_string(
            &PathBuf::from(&post.output_path).join(public_module.clone() + ".js"),
        );
        import_map_str += &format!(
            "        \"{}\": \"file:///{}\",\n",
            &public_module, mod_path
        );
    }
    // Development mode adds error logging, including errors reported by the browser.
    let renderer_js = match build_mode {
        BuildMode::Development => include_str!("../server-renderer.debug.js"),
        BuildMode::Release => include_str!("../server-renderer.js"),
    };
    let server_src = format!(
        include_str!("../server-renderer-template.txt"),
        frontend_main, import_map_str, renderer_js
    );

    let server_source_file = frontend_main + "-renderer.generated.mjs";
    let server_source_file_abs = foil_cache_path.clone().join(&server_source_file);
    let server_source_file_abs_str = clean_path_string(&server_source_file_abs);

    let write_result = fs::write(&server_source_file_abs, server_src);
    if write_result.is_err() {
        println!(
            "❌ Failed to write foil renderer.generated.js in current working directory, aborting."
        );
        return Ok(None);
    }

    let cwd_node_modules = clean_path_string(&cwd.join("node_modules"));
    let builder_node_modules = clean_path_string(&foil_builder_path.join("node_modules"));
    let node_path_str = builder_node_modules + ";" + &cwd_node_modules;

    // One rolling file. Restarting the server used to stamp a new log every time.
    let renderer_log_file_abs = foil_log_path.join("foil-renderer-log.txt");
    let renderer_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&renderer_log_file_abs)
        .unwrap();

    // --no-warnings would swallow --trace-warnings, so only one is ever passed.
    let trace_warnings = *build_mode == BuildMode::Development
        || std::env::var(TRACE_WARNINGS).is_ok_and(|v| !v.is_empty() && v != "0");
    let warning_flag = if trace_warnings {
        "--trace-warnings"
    } else {
        "--no-warnings"
    };

    let mut backend_renderer_child = process::Command::new("node")
        .current_dir(cwd)
        .env("NODE_PATH", node_path_str)
        .args([
            warning_flag,
            &server_source_file_abs_str,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // React runtime errors land on stderr; node also prints on stdout. Both go to the
    // console and the renderer log so `foil_cli server start` shows them live.
    if let Some(stdout) = backend_renderer_child.stdout.take() {
        tee_to_console_and_log(stdout, renderer_log.try_clone().unwrap(), io::stdout());
    }
    if let Some(stderr) = backend_renderer_child.stderr.take() {
        tee_to_console_and_log(stderr, renderer_log, io::stderr());
    }

    Ok(Some(backend_renderer_child))
}
