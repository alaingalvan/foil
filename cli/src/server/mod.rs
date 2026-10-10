mod backend;
mod logging;
mod renderer;

use crate::builder::BuildMode;
use crate::builder::{get_foil_builder_path, get_foil_folder_path};
use crate::error::Result;
use crate::misc::connect_db;
use crate::misc::run_on_sql_stack;
use crate::query_post::query_post;
use async_std::task;
use backend::spawn_backend;
use renderer::spawn_renderer;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Child;

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

pub async fn start_server(build_mode: BuildMode) -> Result<()> {
    match run_on_sql_stack("foil-server", move || {
        task::block_on(spawn_server(build_mode))
    })? {
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
pub async fn spawn_server(build_mode: BuildMode) -> Result<Option<ServerChildren>> {
    let pool = connect_db().await?;
    let cwd = env::current_dir().unwrap_or_default();

    // 📦 Resolve Foil package.json from current working directory, attempt to run server from it:
    match query_post(&pool, "/".to_string()).await {
        Ok(post) => {
            let foil_folder_path = get_foil_folder_path();
            let foil_builder_path = get_foil_builder_path();
            let foil_cache_path = foil_builder_path.join(PathBuf::from("cache"));
            let foil_log_path = foil_folder_path.join(PathBuf::from("log"));

            fs::create_dir_all(&foil_cache_path)?;
            fs::create_dir_all(&foil_log_path)?;

            let mut backend = spawn_backend(&cwd, &foil_log_path)?;

            match spawn_renderer(
                &post,
                &cwd,
                &foil_builder_path,
                &foil_cache_path,
                &foil_log_path,
                &build_mode,
            )
            .await?
            {
                Some(renderer) => Ok(Some(ServerChildren { backend, renderer })),
                None => {
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
pub async fn spawn_renderer_only(build_mode: BuildMode) -> Result<Option<Child>> {
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
    spawn_renderer(
        &post,
        &cwd,
        &foil_builder_path,
        &foil_cache_path,
        &foil_log_path,
        &build_mode,
    )
    .await
}
