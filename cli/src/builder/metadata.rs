use super::get_foil_builder_path;
use super::package_schema::StringMap;
use super::resolver::Foil;
use super::static_assets::FoilFile;
use crate::BuildMode;
use async_std::task::{spawn, JoinHandle};
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{fs, io::Read};

//=====================================================================================================================
/// Metadata for a given foil project.
#[derive(Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoilMetadata {
    /// Version number.
    pub version: u32,
    /// release or development mode.
    pub mode: String,
    /// Source/output files for this foil post.
    pub files: Vec<FoilFile>,
    /// SystemJS version, if applicable.
    pub systemjs_version: String,
    /// Map of public modules and their currently built version.
    pub public_modules: StringMap,
}

//=====================================================================================================================
/// Change status of the given foil metadata.
#[derive(Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoilMetadataStatus {
    /// If any source files have changed.
    pub files_changed: bool,
    /// If the runtime has changed.
    pub runtime_changed: bool,
    /// If any public modules have changed.
    pub public_modules_changed: bool,
}

//=====================================================================================================================
impl FoilMetadataStatus {
    pub fn changed(&self) -> bool {
        self.files_changed || self.runtime_changed || self.public_modules_changed
    }
}

//=====================================================================================================================
impl FoilMetadata {
    /// Open a metadata file, generally named "foil-meta.json".
    pub fn open(path: PathBuf) -> FoilMetadata {
        let mut metadata = FoilMetadata::default();
        if path.exists() {
            let foil_lock_file = fs::File::open(&path);
            let mut contents = String::new();
            match foil_lock_file {
                Ok(mut foil_lock) => {
                    foil_lock.read_to_string(&mut contents).unwrap_or_default();
                    metadata = match serde_json::from_str(&contents) {
                        Ok(v) => v,
                        _ => FoilMetadata::default(),
                    };
                }
                Err(_) => (),
            }
        }
        metadata.files.sort_by(|a, b| a.path.cmp(&b.path));
        metadata
    }

    /// Verify if source files have changed, this can happen if there's additional files or any existing files have been modified.
    fn verify_source_files(&self, foil: &Foil) -> bool {
        let mut source_files = foil.source_files.clone();
        if self.files.len() != source_files.len() || (source_files.is_empty()) {
            return true;
        }
        // Both sorted lists should be equivalent:
        let mut lists_match = true;
        source_files.sort_by(|a, b| a.path.cmp(&b.path));
        for i in 0..=(source_files.len() - 1) {
            let mut path_equal = source_files[i].path.eq(&self.files[i].path);
            path_equal &= source_files[i]
                .modified_date
                .eq(&self.files[i].modified_date);
            path_equal &= PathBuf::from(&self.files[i].path).exists();
            lists_match &= path_equal;
            if !lists_match {
                return true;
            }
        }
        false
    }

    /// Verify if the SystemJS runtime has been updated. This can happen if either the builder uses a newer version of SystemJS.
    fn verify_runtime(&self, foil: &Foil) -> bool {
        if foil.frontend {
            let system_path = foil.output_path.join(PathBuf::from("system.js"));
            let foil_builder_path = get_foil_builder_path();
            let builder_package_path = foil_builder_path.join(PathBuf::from("package.json"));
            let file_result = fs::File::open(&builder_package_path);
            if file_result.is_ok() {
                let mut file = file_result.unwrap();
                let mut contents = String::new();
                let file_read_result = file.read_to_string(&mut contents);
                if file_read_result.is_ok() {
                    let data: serde_json::Value = match serde_json::from_str(&contents) {
                        Ok(v) => v,
                        Err(_er) => serde_json::Value::Null,
                    };
                    let current_systemjs_version = match data["dependencies"]["systemjs"].clone() {
                        serde_json::Value::String(s) => s,
                        _ => "=6.15.1".to_string(),
                    };
                    return self.systemjs_version != current_systemjs_version
                        || !system_path.exists();
                }
            }
            true
        } else {
            false
        }
    }

    /// Verify if public vendor modules such as React, React Router, etc. have been changed, if so we should rebuild them.
    fn verify_public_modules(&self, foil: &Foil) -> bool {
        if !foil.public_modules.is_empty() {
            // Check our metadata for the current version of our public modules. If there's been a change, rebuild.
            let mut module_exists_and_matches = true;
            for (public_module, version) in foil.public_modules_map.iter() {
                let public_module_path = foil
                    .output_path
                    .join(PathBuf::from(public_module.clone() + ".js"));
                module_exists_and_matches &= public_module_path.exists();
                if self.public_modules.contains_key(public_module) {
                    let version_equals = &self.public_modules[public_module].eq(version);
                    module_exists_and_matches &= module_exists_and_matches;
                    module_exists_and_matches &= version_equals;
                    if !module_exists_and_matches {
                        break;
                    }
                } else {
                    module_exists_and_matches &= false;
                }
            }
            !module_exists_and_matches
        } else {
            false
        }
    }

    /// Verify if a given set of assets matches our foil metadata file path/modified dates. Returns true if there are changes.
    pub fn verify(&self, foil: &Foil, build_mode: BuildMode) -> FoilMetadataStatus {
        // 🏗️ If the build mode has changed, force a full rebuild.
        if (self.mode == "release" && build_mode != BuildMode::Release)
            || (self.mode == "development" && build_mode != BuildMode::Development)
        {
            return FoilMetadataStatus {
                files_changed: true,
                runtime_changed: true,
                public_modules_changed: true,
            };
        }

        // 🧱 Verify if source files have changed first:
        let source_files_changed = self.verify_source_files(foil);
        // 🏎️ Verify SystemJS runtime:
        let runtime_changed = self.verify_runtime(foil);
        // 📚 Check if public vendor modules need to be built.
        let public_modules_changed = self.verify_public_modules(foil);

        FoilMetadataStatus {
            files_changed: source_files_changed,
            runtime_changed,
            public_modules_changed,
        }
    }
}

//=====================================================================================================================
/// Write a foil project's corresponding metadata file, used to determine is there's been changes to the project.
pub async fn write_foil_metadata(
    path: PathBuf,
    source_files: Vec<FoilFile>,
    systemjs_version: String,
    public_modules: StringMap,
    build_mode: BuildMode,
) -> JoinHandle<()> {
    spawn(async move {
        let file = fs::File::create(path).unwrap();
        let mut writer = std::io::BufWriter::new(file);
        let metadata = FoilMetadata {
            version: 0,
            files: source_files.to_vec(),
            systemjs_version: systemjs_version.to_string(),
            public_modules: public_modules.clone(),
            mode: (if build_mode == BuildMode::Release {
                "release"
            } else {
                "development"
            })
            .to_string(),
        };
        serde_json::to_writer(&mut writer, &metadata).unwrap();
    })
}
