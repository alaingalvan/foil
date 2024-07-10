pub mod build_mode;

mod database;
mod metadata;
mod nodejs;
pub mod package_schema;
mod resolver;
mod rss;
mod static_assets;

use crate::error::Result;
use crate::misc::connect_db;
pub use build_mode::BuildMode;
use database::{clean_database, udpate_foil_db};
use metadata::{write_foil_metadata, FoilMetadata};
use nodejs::compile_foil_main;
pub use resolver::read_foil_package;
use resolver::{resolve_foils, Foil};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::Child;
use std::time::Instant;

//=====================================================================================================================
/// Process the current working directory for Foil projects.
pub async fn build(build_mode: BuildMode) -> Result<()> {
    // 📚 Configure database...
    let pool = connect_db().await?;
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
    let mut build_children: Vec<Child> = vec![];
    let mut public_module_cache: HashMap<String, Vec<String>> = HashMap::new();
    let root_foil_permalink = "/".to_string();
    for (i, (resolved_foil, foil_metadata)) in resolved_foils.iter_mut().enumerate() {
        println!(
            "\n👟 Processing {} {}/{}...",
            &resolved_foil.title,
            i + 1,
            &resolved_foil_len
        );

        // 🧱 Check if foil has changed.
        let foil_changed = foil_metadata.verify(&resolved_foil, build_mode.clone());

        // Recompile and update the database if there's been changes to source files.
        if foil_changed.changed() {
            // 📅 Write foil post to database.
            let update_future = udpate_foil_db(resolved_foil.clone(), pool.clone());

            // ⏳ Only wait for frontend posts.
            if resolved_foil.frontend {
                let _ = update_future.await;
                if !public_module_cache.contains_key(&root_foil_permalink) {
                    let found: (i32, Vec<String>) =
                        sqlx::query_as("SELECT id, public_modules FROM posts WHERE permalink = $1")
                            .bind(&root_foil_permalink)
                            .fetch_one(&pool)
                            .await
                            .unwrap_or((-1, vec![]));
                    public_module_cache.insert(root_foil_permalink.clone(), found.1);
                }
            } else {
                update_futures.push(update_future);
            }

            // 🛠️ Build foil if needed.
            if resolved_foil.requires_build() {
                // We must inherit public modules from the root foil project.
                if !resolved_foil.frontend {
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
                let child = compile_foil_main(build_mode.clone(), &resolved_foil, foil_changed)?;
                build_children.push(child);
            }

            // 🍥 Write out metadata to local lock file.
            let foil_lock_path = resolved_foil.root_path.join("foil-meta.json");
            let systemjs_version = "=6.14.3".to_string();
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

    // 🌊 Write the RSS output for this foil project.
    rss::build_rss(pool.clone()).await;

    for mut child in build_children {
        child.wait().expect("Failed to run Foil Builder...");
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
