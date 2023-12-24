use chrono::{DateTime, Utc};
use lexiclean::Lexiclean;
use path_slash::PathBufExt;
use serde_derive::{Deserialize, Serialize};
use std::fs;
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
        let mut clean_asset = asset.clone();
        if clean_asset.ends_with("*") {
            clean_asset.remove(asset.len() - 1);
        }
        let cur_asset: PathBuf = PathBuf::from(
            base_path
                .clone()
                .join(clean_asset)
                .lexiclean()
                .to_slash()
                .unwrap()
                .to_string(),
        );
        let read_dir = fs::read_dir(&cur_asset);
        if read_dir.is_err() {
            println!(
                "❌ Couldn't read asset path {}, skipping.",
                &cur_asset.to_str().unwrap_or("")
            );
            continue;
        }
        if read_dir.is_ok() {
            for cur_entry in read_dir.unwrap() {
                let entry = cur_entry.unwrap();
                let entry_path = entry.path();
                let relative_path = entry_path.strip_prefix(base_path.clone()).unwrap();
                let joined_path = PathBuf::from(permalink).join(relative_path);
                let p: String = joined_path
                    .to_str()
                    .unwrap_or("/")
                    .to_string()
                    .replace("\\", "/");
                let meta = return_err!(entry.metadata(), "Failed to get metadata for asset.");
                if meta.is_file() {
                    assets.push(StaticAsset {
                        path: entry_path.to_slash().unwrap().to_string(),
                        permalink: p,
                        modified_date: DateTime::<Utc>::from(meta.modified().unwrap()),
                    });
                }
            }
        }
    }
    Ok(())
}
