pub mod build_mode;

mod database;
mod metadata;
mod nodejs;
mod package_schema;
mod resolver;
mod static_assets;

use crate::error::Result;
use crate::misc::connect_db;
pub use build_mode::BuildMode;
use database::{clean_database, update_foils};
use metadata::{write_foil_metadata, FoilMetadata};
use nodejs::compile_foil_main;
pub use resolver::read_foil_package;
use resolver::{resolve_foils, Foil};
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
    resolve_foils(cwd, build_mode.clone(), &mut resolved_foils).await?;

    // Process resolved foils...
    let resolved_foil_len = resolved_foils.len();
    if resolved_foils.is_empty() {
        println!("👍 No changes found, exiting.")
    } else {
        println!("🎡 Processing {} file(s).", &resolved_foil_len)
    }

    let mut build_children: Vec<Child> = vec![];
    for (i, resolved_foil) in resolved_foils.iter_mut().enumerate() {
        println!("\n👟 Processing foil {}/{}...", i + 1, &resolved_foil_len);

        // 🔒 Load foil-meta file and compare source file path/modified date.
        let foil_lock_path = resolved_foil.root_path.join("foil-meta.json");
        let foil_metadata = FoilMetadata::open(foil_lock_path);

        // 🧱 Check if foil has changed.
        let foil_changed = foil_metadata.verify(&resolved_foil);

        // Recompile and update the database if there's been changes to source files.
        let main_path = PathBuf::from(resolved_foil.main.clone());
        let main_path_file = main_path.file_name().unwrap_or_default();
        let mut main_file_path = PathBuf::from(main_path_file);
        let main_ext = main_file_path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let is_typescript = main_ext == "ts" || main_ext == "tsx";
        if foil_changed.changed() {
            if is_typescript {
                let child = compile_foil_main(&resolved_foil, foil_changed, build_mode.clone())?;
                build_children.push(child);
                // Rewrite foil.main to relative path to permalink:
                main_file_path.set_extension("js");
                let main_file_str = main_file_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                let relative_path = resolved_foil
                    .output_path
                    .strip_prefix(resolved_foil.root_path.clone())
                    .unwrap();
                let joined_path = PathBuf::from(resolved_foil.permalink.clone())
                    .join(relative_path)
                    .join(PathBuf::from(main_file_str));
                let p: String = joined_path
                    .to_str()
                    .unwrap_or("/")
                    .to_string()
                    .replace("\\", "/");
                resolved_foil.main = p;
            }

            // Write foil post to database.
            update_foils(&resolved_foil, &resolved_foil.root_path, pool.clone()).await?;

            // Write out metadata to local lock file.
            let foil_lock_path = resolved_foil.root_path.join("foil-meta.json");
            let systemjs_version = "=6.14.2".to_string();
            write_foil_metadata(
                &foil_lock_path,
                &resolved_foil.source_files,
                &systemjs_version,
                &resolved_foil.public_modules_map,
            );
        }
    }

    for mut child in build_children {
        child.wait().expect("Failed to run Foil Builder...");
    }
    Ok(())
}
