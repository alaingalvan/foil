use chrono::{DateTime, Utc};
use glob::glob;
use lexiclean::Lexiclean;
use path_slash::PathBufExt;
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::Result;
use crate::return_err;

//=====================================================================================================================
/// File/modified date pair.
#[derive(Clone, Serialize, Deserialize, sqlx::Type, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoilFile {
    /// Path to this file or folder glob in the server.
    pub path: String,
    /// When this file was last modified on the server, used to update it.
    pub modified_date: chrono::DateTime<chrono::Utc>,
}

//=====================================================================================================================
/// A static asset stored in the database.
#[derive(Clone, Serialize, Deserialize, sqlx::Type, Debug)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "static_asset")]
pub struct StaticAsset {
    /// Path to this file in the server.
    pub path: String,
    /// The permalink of this file in the website.
    pub permalink: String,
    /// When this file was last modified on the server, used to update it.
    pub modified_date: chrono::DateTime<chrono::Utc>,
}

//=====================================================================================================================
/// Given a directory, convert it to a list of static assets.
pub fn build_static_assets(
    base_path: &PathBuf,
    permalink: &str,
    paths: &Vec<String>,
    assets: &mut Vec<StaticAsset>,
) -> Result<()> {
    for asset in paths {
        let cur_asset = base_path
            .clone()
            .join(asset)
            .lexiclean()
            .to_slash()
            .unwrap()
            .to_string();

        for entry_result in glob(&cur_asset)? {
            match entry_result {
                Ok(entry) => {
                    let relative_path = entry.strip_prefix(base_path.clone()).unwrap();
                    let joined_path = PathBuf::from(permalink).join(relative_path);
                    let p: String = joined_path
                        .to_str()
                        .unwrap_or("/")
                        .to_string()
                        .replace("\\", "/");
                    let meta = return_err!(entry.metadata(), "Failed to get metadata for asset.");
                    if meta.is_file() {
                        assets.push(StaticAsset {
                            path: entry.to_slash().unwrap().to_string(),
                            permalink: p,
                            modified_date: DateTime::<Utc>::from(meta.modified().unwrap()),
                        });
                    }
                }
                Err(_e) => {
                    println!("❌ Couldn't read asset path {}, skipping.", &cur_asset);
                    break;
                }
            }
        }
    }
    Ok(())
}
