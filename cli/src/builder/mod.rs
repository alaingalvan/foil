pub mod build_mode;
pub mod package_schema;

mod database;
mod imports;
mod metadata;
mod nodejs;
mod resolver;
mod server;
mod static_assets;
mod watcher;

use crate::error::Result;
use crate::misc::connect_db;
pub use build_mode::BuildMode;
pub use resolver::read_foil_package;
pub use watcher::watch;

use database::{clean_database, update_foil_db};
use metadata::{FoilMetadata, write_foil_metadata};
use nodejs::compile_foil_main;
use resolver::{Foil, resolve_foils};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use server::{BuilderServer, PendingBuild};
use sqlx::{Pool, Postgres};
use std::time::Instant;

/// Foils whose metadata says a source, runtime, or public module changed.
pub fn changed_foils(resolved: Vec<(Foil, FoilMetadata)>, build_mode: &BuildMode) -> Vec<Foil> {
    resolved
        .into_iter()
        .filter_map(|(foil, metadata)| {
            if metadata.verify(&foil, build_mode.clone()).changed() {
                Some(foil)
            } else {
                None
            }
        })
        .collect()
}

//=====================================================================================================================
type Session = (Pool<Postgres>, BuilderServer);

/// Database pool and builder server, kept alive across rebuilds.
pub struct BuildSession {
    build_mode: BuildMode,
    state: Option<Session>,
}

impl BuildSession {
    pub fn new(build_mode: BuildMode) -> Self {
        Self {
            build_mode,
            state: None,
        }
    }

    /// Process the current working directory for Foil projects.
    pub async fn build(&mut self) -> Result<()> {
        let build_mode = self.build_mode.clone();
        let state = self.state.take();
        // `spawn` still polls on async-std's 1 MiB worker stacks. A dedicated thread
        // gets its own runtime, so the sqlx future is polled on this larger stack.
        let (state, result) = crate::misc::run_on_sql_stack("foil-build", move || {
            Ok(async_std::task::block_on(build_on_thread(build_mode, state)))
        })?;
        self.state = state;
        result
    }
}

/// Process the current working directory once, without keeping anything alive.
pub async fn build(build_mode: BuildMode) -> Result<()> {
    BuildSession::new(build_mode).build().await
}

async fn build_on_thread(
    build_mode: BuildMode,
    state: Option<Session>,
) -> (Option<Session>, Result<()>) {
    let session = match state {
        Some(session) if session.1.is_running() => session,
        _ => match open_session(&build_mode).await {
            Ok(session) => session,
            Err(e) => return (None, Err(e)),
        },
    };
    let result = run_build(&build_mode, &session.0, &session.1).await;
    (Some(session), result)
}

async fn open_session(build_mode: &BuildMode) -> Result<Session> {
    // 📚 Configure database...
    let pool = connect_db().await?;
    let server = BuilderServer::start(build_mode)?;
    Ok((pool, server))
}

async fn run_build(
    build_mode: &BuildMode,
    pool: &Pool<Postgres>,
    builder_server: &BuilderServer,
) -> Result<()> {
    let cwd = env::current_dir().unwrap_or_default();

    // ⏳ Start build benchmark:
    let now = Instant::now();

    // 🧼🫧 Clean the database and remove any currently missing foils.
    clean_database(pool.clone()).await?;

    let mut resolved_foils: Vec<(Foil, FoilMetadata)> = vec![];
    resolve_foils(cwd, &mut resolved_foils).await?;

    // Process resolved foils...
    let resolved_foil_len = resolved_foils.len();
    if resolved_foils.is_empty() {
        println!("👍 No changes found, exiting.")
    } else {
        println!("🎡 Processing {} file(s).", &resolved_foil_len)
    }

    let mut update_futures = vec![];
    let mut write_futures = vec![];
    let mut build_children: Vec<PendingBuild> = vec![];
    let mut public_module_cache: HashMap<String, Vec<String>> = HashMap::new();
    let root_foil_permalink = "/".to_string();
    for (i, (resolved_foil, foil_metadata)) in resolved_foils.iter_mut().enumerate() {
        // 🧱 Check if foil has changed.
        let foil_changed = foil_metadata.verify(&resolved_foil, build_mode.clone());

        // Recompile and update the database if there's been changes to source files.
        if foil_changed.changed() {
            println!(
                "\n👟 Processing {} {}/{}...",
                &resolved_foil.title,
                i + 1,
                &resolved_foil_len
            );
            // 📅 Write foil post to database.
            let update_future = update_foil_db(resolved_foil.clone(), pool.clone());

            // ⏳ Only wait for frontend posts.
            if resolved_foil.frontend {
                let _ = update_future.await;
            } else {
                update_futures.push(update_future);
            }

            // 🛠️ Build foil if needed.
            if resolved_foil.requires_build() {
                // We must inherit public modules from the root foil project.
                if !resolved_foil.frontend {
                    if !public_module_cache.contains_key(&root_foil_permalink) {
                        let found: (i32, Vec<String>) = sqlx::query_as(
                            "SELECT id, public_modules FROM posts WHERE permalink = $1",
                        )
                        .bind(&root_foil_permalink)
                        .fetch_one(pool)
                        .await
                        .unwrap_or((-1, vec![]));
                        public_module_cache.insert(root_foil_permalink.clone(), found.1);
                    }
                    let cached_public_modules = public_module_cache.get(&root_foil_permalink);
                    match cached_public_modules {
                        Some(v) => {
                            for parent_module in v {
                                resolved_foil.public_modules.push(parent_module.to_string());
                            }
                        }
                        _ => (),
                    }
                }
                // Build project.
                let child = compile_foil_main(
                    build_mode.clone(),
                    &resolved_foil,
                    foil_changed,
                    builder_server,
                )?;
                build_children.push(child);
            }

            // 🍥 Write out metadata to local lock file.
            let foil_lock_path = resolved_foil.root_path.join("foil-meta.json");
            let systemjs_version = "=6.15.1".to_string();
            let write_future = write_foil_metadata(
                foil_lock_path,
                resolved_foil.source_files.clone(),
                systemjs_version,
                resolved_foil.public_modules_map.clone(),
                build_mode.clone(),
            );
            write_futures.push(write_future);
        }
    }

    // Join all async threads here.
    futures::future::join_all(update_futures).await;
    futures::future::join_all(write_futures).await;

    let mut failed = None;
    for pending in build_children {
        if let Err(e) = pending.wait() {
            println!("\n❌ {}", e);
            failed = Some(e);
        }
    }
    if let Some(e) = failed {
        return Err(e);
    }
    let elapsed = now.elapsed();
    println!("⏲️ Build time: {:.2?}", elapsed);
    Ok(())
}
//=====================================================================================================================
/// Get the foil folder path.
pub fn get_foil_folder_path() -> PathBuf {
    let foil_exe_path = env::current_exe().unwrap_or_default();
    let foil_folder_path = foil_exe_path.parent().unwrap();
    foil_folder_path.to_path_buf()
}

//=====================================================================================================================
/// Get the builder folder path.
pub fn get_foil_builder_path() -> PathBuf {
    let foil_builder_path = get_foil_folder_path().join(PathBuf::from("builder"));
    foil_builder_path
}
