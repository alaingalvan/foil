use crate::error::Result;
use crate::misc::{get_db_url, DATABASE_URL};
use std::env;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::{self, Child, Stdio};

// NPM is somewhat buggy at times, and requires the extension on windows.
#[cfg(windows)]
const FOIL_BACKEND: &'static str = "foil_backend.exe";

#[cfg(not(windows))]
const FOIL_BACKEND: &'static str = "foil_backend";

/// Start the GraphQL backend. Its stderr is appended to one log across restarts.
pub(super) fn spawn_backend(cwd: &PathBuf, foil_log_path: &PathBuf) -> Result<Child> {
    let backend_log_file_abs = foil_log_path.join("foil-backend-log.txt");
    let backend_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&backend_log_file_abs)
        .unwrap();

    let child = process::Command::new(&FOIL_BACKEND)
        .current_dir(cwd)
        .stdin(process::Stdio::null())
        .env_remove("args")
        .env(DATABASE_URL, get_db_url())
        .env("RUST_LOG", env::var("RUST_LOG").unwrap_or_default())
        .stderr(Stdio::from(backend_file))
        .spawn()
        .unwrap();
    Ok(child)
}
