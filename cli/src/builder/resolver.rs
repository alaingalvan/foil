use super::nodejs::find_all_imports;
use super::package_schema::{FoilRedirect, NodeAuthor, NodePackage, StringMap};
use super::static_assets::{build_static_assets, FoilFile, StaticAsset};
use crate::{return_err, Result};
use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
use walkdir::{DirEntry, WalkDir};

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
    pub rss: Vec<String>,

    /// List of redirects.
    pub redirects: Vec<FoilRedirect>,
}

impl Foil {
    /// Determine if the current foil project requires a build.
    pub fn requires_build(&self) -> bool {
        let main_path = PathBuf::from(self.main.clone());
        let main_path_file = main_path.file_name().unwrap_or_default();
        let main_file_path = PathBuf::from(main_path_file);
        let main_ext = main_file_path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        main_ext == "ts" || main_ext == "tsx" || main_ext == "mdx"
    }

    /// Resolve the final permalink path for this foil
    pub fn resolve_js_main(&self) -> String {
        let main_path = PathBuf::from(self.main.clone());
        let main_path_file = main_path.file_name().unwrap_or_default();
        let mut main_file_path = PathBuf::from(main_path_file);
        main_file_path.set_extension("js");
        let main_file_str = main_file_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let relative_path = self
            .output_path
            .strip_prefix(self.root_path.clone())
            .unwrap();
        let joined_path = PathBuf::from(self.permalink.clone())
            .join(relative_path)
            .join(PathBuf::from(main_file_str));
        let p: String = joined_path
            .to_str()
            .unwrap_or("/")
            .to_string()
            .replace("\\", "/");
        p
    }
}

//=====================================================================================================================
/// ❌ Determine if a directory entry is a foil project. Skip folders/files used when building (node_modules, target, hidden folders):
fn is_foil_package(entry: &DirEntry) -> bool {
    let file_name = entry.file_name().to_str().unwrap_or_default();
    if file_name.len() <= 0
        || file_name == "node_modules"
        || file_name == "target"
        || file_name.char_indices().next().unwrap().1 == '.'
    {
        return false;
    }
    true
}

//=====================================================================================================================
/// Traverse a given folder and its files for foil projects, and process them.
pub async fn resolve_foils(path: PathBuf, resolved_foils: &mut Vec<Foil>) -> Result<()> {
    // Recursively find all foil modules.
    for entry in WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| is_foil_package(e))
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_str().unwrap_or_default();
        if file_name != "package.json" {
            continue;
        }
        // 🌟 We've found a foil project, attempt to foilify it and process it later.
        let cur_path_package = entry.into_path();
        let cur_path_root = cur_path_package.parent().unwrap().to_path_buf();
        match read_foil_package(&cur_path_package) {
            Ok(v) => {
                match process_foil_project(v, &cur_path_root) {
                    Ok(resolved_foil) => {
                        resolved_foils.push(resolved_foil);
                    }
                    Err(er) => {
                        println!("Failed to process Foil project. \n{:?}", er);
                    }
                };
            }
            Err(_er) => (),
        };
    }

    // Sort them by non-frontend, then frontend.
    resolved_foils.sort_by(|a, b| {
        if a.frontend && !b.frontend {
            Ordering::Less
        } else if !a.frontend && b.frontend {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    });

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
    for asset in assets.iter() {
        let asset_path = PathBuf::from(asset.path.clone());
        let mut asset_file_name = asset_path.clone();
        asset_file_name.set_extension("");
        let file_name = asset_file_name.file_name().unwrap_or_default();
        if file_name == "cover" && asset_path.extension().unwrap_or_default() != "svg" {
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
        redirects: package.foil.redirects,
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
