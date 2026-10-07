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
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{self, Child, Stdio};
use std::thread;

fn clean_path_string(p: &std::path::PathBuf) -> String {
    p.clone()
        .lexiclean()
        .to_slash()
        .unwrap()
        .to_string()
        .replace("\\", "/")
}

fn tee_to_console_and_log<R: Read + Send + 'static>(
    mut reader: R,
    mut log: File,
    mut console: impl Write + Send + 'static,
) {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = console.write_all(&buf[..n]);
                    let _ = console.flush();
                    let _ = log.write_all(&buf[..n]);
                    let _ = log.flush();
                }
            }
        }
    });
}

//=====================================================================================================================
// NPM is somewhat buggy at times, and requires the extension on windows.
#[cfg(windows)]
const FOIL_BACKEND: &'static str = "foil_backend.exe";

#[cfg(not(windows))]
const FOIL_BACKEND: &'static str = "foil_backend";

pub struct ServerChildren {
    pub backend: Child,
    pub renderer: Child,
}

/// Stop a child if it is still running. Used when watch mode restarts the renderer.
pub fn stop_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub async fn start_server(_build_mode: BuildMode) -> Result<()> {
    match spawn_server().await? {
        Some(mut children) => {
            children
                .renderer
                .wait()
                .expect("❌ Failed to run Foil Renderer...");
            children
                .backend
                .wait()
                .expect("❌ Failed to run Foil Backend...");
        }
        None => (),
    }
    Ok(())
}

/// Spawn the GraphQL backend and the Node server renderer without waiting for them.
/// Returns `None` when the root post cannot be loaded or the renderer source cannot be written.
pub async fn spawn_server() -> Result<Option<ServerChildren>> {
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
            let date_now_string = log_stamp();
            let backend_log_file = format!("foil-backend-log-{}.txt", &date_now_string);
            let backend_log_file_abs = foil_log_path.clone().join(&backend_log_file);
            let backend_file = File::create(backend_log_file_abs).unwrap();
            let backend_stdio = Stdio::from(backend_file);

            // 🌐 Spawn child processes for the server:
            let foil_database_url = get_db_url();
            let backend_server_child = process::Command::new(&FOIL_BACKEND)
                .current_dir(&cwd)
                .stdin(process::Stdio::null())
                .env_remove("args")
                .env(DATABASE_URL, foil_database_url)
                .env("RUST_LOG", env::var("RUST_LOG").unwrap_or_default())
                .stderr(backend_stdio)
                .spawn()
                .unwrap();

            match spawn_renderer(&post, &cwd, &foil_builder_path, &foil_cache_path, &foil_log_path)
                .await?
            {
                Some(renderer) => Ok(Some(ServerChildren {
                    backend: backend_server_child,
                    renderer,
                })),
                None => {
                    let mut backend = backend_server_child;
                    stop_child(&mut backend);
                    Ok(None)
                }
            }
        }
        Err(e) => {
            println!("Failed to connect to database when starting server.\n{}", e);
            Ok(None)
        }
    }
}

/// Rewrite the generated renderer and start a new Node process. Does not touch the backend.
pub async fn spawn_renderer_only() -> Result<Option<Child>> {
    let pool = connect_db().await?;
    let cwd = env::current_dir().unwrap_or_default();
    let post = match query_post(&pool, "/".to_string()).await {
        Ok(post) => post,
        Err(e) => {
            println!("Failed to connect to database when starting server.\n{}", e);
            return Ok(None);
        }
    };
    let foil_builder_path = get_foil_builder_path();
    let foil_folder_path = get_foil_folder_path();
    let foil_cache_path = foil_builder_path.join(PathBuf::from("cache"));
    let foil_log_path = foil_folder_path.join(PathBuf::from("log"));
    fs::create_dir_all(&foil_cache_path)?;
    fs::create_dir_all(&foil_log_path)?;
    spawn_renderer(&post, &cwd, &foil_builder_path, &foil_cache_path, &foil_log_path).await
}

fn log_stamp() -> String {
    let dt = Utc::now();
    let naive_utc = dt.naive_utc();
    let offset = dt.offset().clone();
    let date_now = DateTime::<Utc>::from_naive_utc_and_offset(naive_utc, offset);
    date_now
        .to_string()
        .replace(":", "_")
        .replace(" ", "_")
        .replace(".", "_")
}

async fn spawn_renderer(
    post: &crate::query_post::SQLPost,
    cwd: &PathBuf,
    foil_builder_path: &PathBuf,
    foil_cache_path: &PathBuf,
    foil_log_path: &PathBuf,
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
        return Ok(None);
    }

    let cwd_node_modules = clean_path_string(&cwd.join("node_modules"));
    let builder_node_modules = clean_path_string(&foil_builder_path.join("node_modules"));
    let node_path_str = builder_node_modules + ";" + &cwd_node_modules;

    let renderer_log_file = format!("foil-renderer-log-{}.txt", log_stamp());
    let renderer_log_file_abs = foil_log_path.clone().join(&renderer_log_file);
    let renderer_log = File::create(&renderer_log_file_abs).unwrap();

    let mut backend_renderer_child = process::Command::new("node")
        .current_dir(cwd)
        .env("NODE_PATH", node_path_str)
        .args([
            "--experimental-specifier-resolution=node",
            "--experimental-modules",
            "--experimental-import-meta-resolve",
            "--no-warnings",
            "--trace-warnings",
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

#[cfg(test)]
mod tests {
    use super::tee_to_console_and_log;
    use std::fs::{self, File};
    use std::io::Cursor;
    use std::time::{Duration, Instant};

    #[test]
    fn tee_writes_log_and_console() {
        let dir = std::env::temp_dir().join(format!(
            "foil-tee-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("renderer.log");
        let console_path = dir.join("console.txt");
        let log = File::create(&log_path).unwrap();
        let console = File::create(&console_path).unwrap();

        tee_to_console_and_log(Cursor::new(b"react boom\n".to_vec()), log, console);

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut logged = String::new();
        while Instant::now() < deadline {
            logged = fs::read_to_string(&log_path).unwrap_or_default();
            if logged.contains("react boom") {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let consoled = fs::read_to_string(&console_path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(logged, "react boom\n");
        assert_eq!(consoled, "react boom\n");
    }
}
