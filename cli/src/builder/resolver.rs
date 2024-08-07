use super::metadata::FoilMetadata;
use super::nodejs::find_all_imports;
use super::package_schema::{NodeAuthor, NodePackage, StringMap};
use super::static_assets::{build_static_assets, FoilFile, StaticAsset};
use crate::Result;
use async_std::task::{spawn, JoinHandle};
use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
use walkdir::{DirEntry, WalkDir};

//=====================================================================================================================
// Foil structure indexed in database.
#[derive(Clone)]
pub struct Foil {
    /// Package name, used for import resolution.
    pub name: String,

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

    /// The covers of this foil post, sorted by compatibility (svg last, jpg first).
    pub covers: Vec<String>,

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
        let relative_path = self
            .output_path
            .strip_prefix(self.root_path.clone())
            .unwrap();
        let joined_path = PathBuf::from(self.permalink.clone())
            .join(relative_path)
            .join(PathBuf::from("main.js"));
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
pub async fn resolve_foil(cur_path_package: PathBuf) -> JoinHandle<Option<(Foil, FoilMetadata)>> {
    spawn(async move {
        let cur_path_root = cur_path_package.parent().unwrap().to_path_buf();

        // 🌟 We've found a foil project, attempt to foilify it and process it later.
        match read_foil_package(&cur_path_package) {
            Ok(v) => {
                match process_foil_project(v, &cur_path_root) {
                    Ok(resolved_foil) => {
                        // 🔒 Load foil-meta file and compare source file path/modified date.
                        let foil_lock_path = resolved_foil.root_path.join("foil-meta.json");
                        let foil_metadata = FoilMetadata::open(foil_lock_path);
                        return Some((resolved_foil, foil_metadata));
                    }
                    Err(_er) => (),
                };
            }
            Err(_er) => (),
        };
        None
    })
}

//=====================================================================================================================
/// Traverse a given folder and its files for foil projects, and process them.
pub async fn resolve_foils(
    path: PathBuf,
    resolved_foils: &mut Vec<(Foil, FoilMetadata)>,
) -> Result<()> {
    let mut resolved_foil_futures = vec![];
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

        resolved_foil_futures.push(resolve_foil(entry.into_path()));
    }

    // Join threads:
    let joined_futures = futures::future::join_all(resolved_foil_futures).await;
    for foil_future in joined_futures {
        match foil_future.await {
            Some(v) => resolved_foils.push(v),
            None => (),
        }
    }

    // Sort them by non-frontend, then frontend.
    // TODO: They should be sorted by depth, and frontend deprecated.
    resolved_foils.sort_by(|a, b| {
        if a.0.frontend && !b.0.frontend {
            Ordering::Less
        } else if !a.0.frontend && b.0.frontend {
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

    // Resolve the covers or leave it as an empty string.
    let mut assets: Vec<StaticAsset> = vec![];
    build_static_assets(
        path,
        &package.foil.permalink,
        &package.foil.assets,
        &mut assets,
    )?;
    let mut covers: Vec<String> = vec![];
    for asset in assets.iter() {
        let asset_path = PathBuf::from(asset.path.clone());
        let mut asset_file_name = asset_path.clone();
        asset_file_name.set_extension("");
        let file_name = asset_file_name.file_name().unwrap_or_default();
        let extension = asset_path.extension().unwrap_or_default();
        if file_name == "cover"
            && (extension == "jpg"
                || extension == "svg"
                || extension == "gif"
                || extension == "png"
                || extension == "mp4")
        {
            if extension == "svg" {
                covers.push(asset.permalink.clone());
            } else {
                let covers_copy = covers.clone();
                covers = vec![asset.permalink.clone()];
                for cover in covers_copy {
                    covers.push(cover);
                }
            }
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

    let name = if package.name.len() > 0 {
        package.name
    } else {
        package
            .foil
            .title
            .clone()
            .replace("-", "")
            .replace("  ", " ")
            .replace(" ", "-")
            .replace("_", "-")
            .to_lowercase()
    };

    // Finalize resolved foil for processing:
    let foil = Foil {
        name,
        permalink: package.foil.permalink,
        title: package.foil.title,
        description: package.description,
        authors: authors,
        keywords: package.keywords,
        covers,
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
    let file = match File::open(&file_path) {
        Ok(v) => v,
        Err(e) => {
            println!("{:?}", e);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Failed to open file path to 'package.json'.",
            )));
        }
    };
    let reader = BufReader::new(file);
    let data: NodePackage = match serde_json::from_reader(reader) {
        Ok(v) => v,
        Err(_er) => {
            // TODO: We should probably leave this to some verbose mode...
            println!("Failed to parse package.json, skipping. {:?}", _er);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Failed to parse package.json, skipping.",
            )));
        }
    };
    Ok(data)
}
