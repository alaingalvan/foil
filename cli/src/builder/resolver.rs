use super::nodejs::find_all_imports;
use super::package_schema::{NodeAuthor, NodePackage, StringMap};
use super::static_assets::{build_static_assets, FoilFile, StaticAsset};
use crate::{return_err, BuildMode, Result};
use async_recursion::async_recursion;
use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

//=====================================================================================================================
// Foil structure indexed in database.
pub struct Foil {
    /// The permalink of this post, where it exists in the website.
    pub permalink: String,

    /// The title of this post, used to update the webpage title and web scrapper data.
    pub title: String,

    // The description of this foil post.
    pub description: String,

    /// The author of this foil post.
    pub authors: Vec<NodeAuthor>,

    /// Keywords used for search engine crawlers for this foil post.
    pub keywords: Vec<String>,

    /// The cover of this foil post.
    pub cover: String,

    /// URL to main file for this foil project.
    pub main: String,

    /// The date this post was published. If not present, it's auto-filled with the current date.
    pub date_published: DateTime<Utc>,

    /// The date this post was modified. If not present, it's auto-filled with the current date.
    pub date_modified: DateTime<Utc>,

    /// Static assets tied to this foil post, or relative paths to public assets.
    pub assets: Vec<String>,

    /// Public dependencies this foil post exposes.
    pub public_modules: Vec<String>,

    /// Source files resolved from the main entry file.
    pub source_files: Vec<FoilFile>,

    /// Absolute root path of this foil module on the server.
    pub root_path: PathBuf,

    /// Final absolute output path for this foil module.
    pub output_path: PathBuf,

    /// If this is a frontend foil module.
    pub frontend: bool,

    /// Resolved public modules and their corresponding version.
    pub public_modules_map: StringMap,

    /// The permalink glob to generate this project's RSS feed.
    pub rss: String,
}

//=====================================================================================================================
/// Traverse a given folder and its files for foil projects, and process them.
pub async fn resolve_foils(
    path: PathBuf,
    build_mode: BuildMode,
    resolved_foils: &mut Vec<Foil>,
) -> Result<()> {
    // Recursively find all foil modules.
    // TODO: Currently this is somewhat slow, we should debug to find out why.
    resolve_foils_recursive(path, build_mode, resolved_foils).await?;

    // Sort them by non-frontend, then frontend.
    resolved_foils.sort_by(|a, b| {
        if a.frontend && !b.frontend {
            Ordering::Greater
        } else if !a.frontend && b.frontend {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    });

    Ok(())
}

//=====================================================================================================================
/// Recurse down a given path to find Foil modules.
#[async_recursion]
pub async fn resolve_foils_recursive(
    path: PathBuf,
    build_mode: BuildMode,
    resolved_foils: &mut Vec<Foil>,
) -> Result<()> {
    // 📂 Read all files in the current path.
    let reading_dir = fs::read_dir(&path);
    if reading_dir.is_err() {
        println!(
            "❌ Couldn't process current directory, skipping.\n ❌ Directory: {}",
            &path.to_str().unwrap_or("")
        );
        return Ok(());
    }
    // 🌲 Traverse to a subdirectory if the current directory does not contain a foil package.json.
    // As a rule, no foil directory can contain foil directories.
    let mut files: Vec<PathBuf> = reading_dir
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort_by(|a, b| {
        if a.is_dir() && !b.is_dir() {
            Ordering::Greater
        } else if !a.is_dir() && b.is_dir() {
            Ordering::Less
        } else {
            {
                Ordering::Equal
            }
        }
    });

    // 📂 Process all files in this folder:
    let mut foil_folder = false;
    for file_path in files {
        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        // ❌ Skip folders/files used when building (node_modules, target, hidden folders):
        if file_name.len() <= 0
            || file_name == "node_modules"
            || file_name == "target"
            || file_name.char_indices().next().unwrap().1 == '.'
        {
            continue;
        }

        // 🌠 We've found a package.json file, but is it a Foil package?
        if file_name == "package.json" {
            let data = match read_foil_package(&file_path) {
                Ok(v) => v,
                Err(_er) => {
                    continue;
                }
            };
            // 🛑 Stop processing further subfolders if this is not a frontend module.
            foil_folder = data.foil.frontend;

            // 🌟 We've found a foil project, attempt to foilify it and process it later.
            match process_foil_project(data, &path) {
                Ok(resolved_foil) => {
                    resolved_foils.push(resolved_foil);
                }
                Err(er) => {
                    println!("Failed to process Foil project. \n{:?}", er);
                    continue;
                }
            };
        }

        if file_path.is_dir() && !foil_folder {
            resolve_foils_recursive(file_path.clone(), build_mode.clone(), resolved_foils).await?;
        }
    }
    Ok(())
}

//=====================================================================================================================
/// Process a given foil package and resolve it.
fn process_foil_project(package: NodePackage, path: &PathBuf) -> Result<Foil> {
    // 🚢 Resolve source file imports:
    let source_files = find_all_imports(package.main.clone(), &path);

    // 🚪 Determine output path, can either be current foil package path, or specified by the project configuration.
    let mut output_path = path.clone();
    if !package.foil.output_path.is_empty() {
        let foil_output_path = package.foil.output_path;
        output_path = PathBuf::from_str(&foil_output_path)?;
        if output_path.is_relative() {
            output_path = path.join(output_path);
        }
    }

    // 📅 Either the current date or the date written in the foil package is the published date.
    let date_published = DateTime::<Utc>::from(package.foil.date_published);
    // The most recent modified source file is the public modified date.
    let date_modified = source_files
        .iter()
        .fold(DateTime::<Utc>::MIN_UTC, |acc, a| {
            if a.modified_date.cmp(&acc) == Ordering::Greater {
                a.modified_date
            } else {
                acc
            }
        });

    // Resolve the cover or leave it as an empty string.
    let mut assets: Vec<StaticAsset> = vec![];
    build_static_assets(
        path,
        &package.foil.permalink,
        &package.foil.assets,
        &mut assets,
    )?;
    let mut cover = "".to_string();
    for ref asset in assets.as_slice() {
        let asset_path = PathBuf::from(asset.path.clone());
        if asset_path.ends_with("cover.jpg") {
            cover = asset.permalink.clone();
        }
    }

    // Get public module versions:
    let mut public_modules_map = std::collections::HashMap::<String, String>::new();
    if !package.foil.public_modules.is_empty() {
        for m in package.foil.public_modules.iter() {
            let dependencies = package.dependencies.clone().unwrap_or_default();
            if !dependencies.is_empty() {
                if dependencies.contains_key(m) {
                    let pair = dependencies.get_key_value(m).unwrap();
                    public_modules_map.insert(pair.0.to_string(), pair.1.to_string());
                }
            }
            let dev_dependencies = package.dev_dependencies.clone().unwrap_or_default();
            if !dev_dependencies.is_empty() {
                if dev_dependencies.contains_key(m) {
                    let pair = dev_dependencies.get_key_value(m).unwrap();
                    public_modules_map.insert(pair.0.to_string(), pair.1.to_string());
                }
            }
        }
    }

    let mut authors = vec![package.author];
    if !package.contributors.is_empty() {
        authors.append(&mut package.contributors.clone());
    }

    // Finalize resolved foil for processing:
    let foil = Foil {
        permalink: package.foil.permalink,
        title: package.foil.title,
        description: package.description,
        authors: authors,
        keywords: package.keywords,
        cover,
        main: package.main,
        public_modules: package.foil.public_modules,
        date_published,
        date_modified,
        assets: package.foil.assets,
        source_files,
        root_path: path.clone(),
        output_path,
        frontend: package.foil.frontend,
        public_modules_map,
        rss: package.foil.rss,
    };

    Ok(foil)
}

//=====================================================================================================================
/// Read a given file as a foil package.
pub fn read_foil_package(file_path: &PathBuf) -> Result<NodePackage> {
    let file = return_err!(
        File::open(&file_path),
        "Failed to open file path to 'package.json'."
    );
    let reader = BufReader::new(file);
    let data: NodePackage = match serde_json::from_reader(reader) {
        Ok(v) => v,
        Err(_er) => {
            // TODO: We should probably leave this to some verbose mode...
            println!("Failed to parse package.json, skipping. {:?}", _er);
            return crate::error::err("Failed to deserialize node package from 'package.json");
        }
    };
    Ok(data)
}
