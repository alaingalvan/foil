pub mod build_mode;

mod database;
mod metadata;
mod nodejs;
mod package_schema;
mod resolver;
mod rss;
mod static_assets;

use crate::error::Result;
use crate::misc::connect_db;
pub use build_mode::BuildMode;
use database::{clean_database, update_foils};
use metadata::{write_foil_metadata, FoilMetadata};
use nodejs::compile_foil_main;
pub use resolver::read_foil_package;
use resolver::{resolve_foils, Foil};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::Child;

//=====================================================================================================================
/// Process the current working directory for Foil projects.
pub async fn build(build_mode: BuildMode) -> Result<()> {
    // 📚 Configure database...
    let pool = connect_db().await?;
    let cwd = env::current_dir().unwrap_or_default();

    // 🧼🫧 Clean the database and remove any currently missing foils.
    clean_database(pool.clone()).await?;

    let mut resolved_foils: Vec<Foil> = vec![];
    resolve_foils(cwd, &mut resolved_foils).await?;

    // Process resolved foils...
    let resolved_foil_len = resolved_foils.len();
    if resolved_foils.is_empty() {
        println!("👍 No changes found, exiting.")
    } else {
        println!("🎡 Processing {} file(s).", &resolved_foil_len)
    }

    let mut build_children: Vec<Child> = vec![];
    let mut public_module_cache: HashMap<String, Vec<String>> = HashMap::new();
    for (i, resolved_foil) in resolved_foils.iter_mut().enumerate() {
        println!(
            "\n👟 Processing {} {}/{}...",
            &resolved_foil.title,
            i + 1,
            &resolved_foil_len
        );

        // 🔒 Load foil-meta file and compare source file path/modified date.
        let foil_lock_path = resolved_foil.root_path.join("foil-meta.json");
        let foil_metadata = FoilMetadata::open(foil_lock_path);

        // 🧱 Check if foil has changed.
        let foil_changed = foil_metadata.verify(&resolved_foil, build_mode.clone());

        // Recompile and update the database if there's been changes to source files.
        if foil_changed.changed() {
            // 📅 Write foil post to database.
            update_foils(&resolved_foil, &resolved_foil.root_path, pool.clone()).await?;

            // 🛠️ Build foil if needed.
            if resolved_foil.requires_build() {
                // We must inherit public modules from the root foil project.
                let root_foil_permalink = "/".to_string();
                if !resolved_foil.frontend {
                    if !public_module_cache.contains_key(&root_foil_permalink) {
                        let found: (i32, Vec<String>) = sqlx::query_as(
                            "SELECT id, public_modules FROM posts WHERE permalink = $1",
                        )
                        .bind(&root_foil_permalink)
                        .fetch_one(&pool)
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
                let child = compile_foil_main(build_mode.clone(), &resolved_foil, foil_changed)?;
                build_children.push(child);
            }

            // 🍥 Write out metadata to local lock file.
            let foil_lock_path = resolved_foil.root_path.join("foil-meta.json");
            let systemjs_version = "=6.14.2".to_string();
            write_foil_metadata(
                &foil_lock_path,
                &resolved_foil.source_files,
                &systemjs_version,
                &resolved_foil.public_modules_map,
                build_mode.clone(),
            );
        }
    }

    // 🌊 Write the RSS output for this foil project.
    rss::build_rss(pool.clone()).await;

    for mut child in build_children {
        child.wait().expect("Failed to run Foil Builder...");
    }
    Ok(())
}

//=====================================================================================================================
/// Get the builder folder path.
pub fn get_foil_builder_path() -> PathBuf {
    let foil_builder_path = env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap()
        .join(PathBuf::from("builder"));
    foil_builder_path
}
