use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};
pub type StringMap = std::collections::HashMap<String, String>;

//=====================================================================================================================
/// 📦 A Foil node package.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NodePackage {
    /// The name of this package, must be in snake-case.
    #[serde(default = "default_empty_str")]
    pub name: String,
    /// The single author of this node.js package.json project.
    pub author: NodeAuthor,

    /// Additional named authors of this node.js package.json project.
    #[serde(default = "default_empty_vec")]
    pub contributors: Vec<NodeAuthor>,

    /// Description of this package.
    #[serde(default = "default_empty_str")]
    pub description: String,

    /// Keywords used when searching for this package.
    #[serde(default = "default_empty_vec")]
    pub keywords: Vec<String>,

    /// Main file of this package. With foil packages, this is your source main (eg. src/main.ts).
    pub main: String,

    /// The files to process for this package. If empty it's auto-filled with whatever exists in the package.json directory.
    pub files: Option<Vec<String>>,

    /// Dev dependencies, used when this foil module has public modules.
    pub dev_dependencies: Option<StringMap>,

    /// Dependencies, used when this foil module has public modules.
    pub dependencies: Option<StringMap>,

    /// The Foil object tied to this package.
    pub foil: FoilConfig,
}

//=====================================================================================================================
/// Node.js authors.
#[derive(Serialize, Deserialize, Clone, Debug, sqlx::Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeAuthor {
    pub name: String,
    #[serde(default = "default_empty_str")]
    pub email: String,
    #[serde(default = "default_empty_str")]
    pub url: String,
}

//=======================================================================================================================
/// ✨ A Foil post configuration data.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoilConfig {
    /// The permalink of this post, where it exists in the website.
    pub permalink: String,

    /// The title of this post, used to update the webpage title and web scrapper data.
    pub title: String,

    /// The date this post was published. If not present, it's auto-filled with the current date.
    #[serde(default = "default_current_date")]
    pub date_published: DateTime<Utc>,

    /// Public assets for this foil post.
    #[serde(default = "default_assets")]
    pub assets: Vec<String>,

    /// Output public modules exposed for systemJS.
    #[serde(default = "default_empty_vec")]
    pub public_modules: Vec<String>,

    /// Output file for any compiled files for this foil package.
    #[serde(default = "default_empty_str")]
    pub output_path: String,

    /// is this a Foil frontend? If so it's compiled with public modules exposed with SystemJS.
    #[serde(default = "default_false")]
    pub frontend: bool,

    /// RSS glob path to export posts from.
    #[serde(default = "default_rss_vec")]
    pub rss: Vec<String>,
}

//=====================================================================================================================

fn default_false() -> bool {
    false
}

fn default_empty_str() -> String {
    "".to_string()
}

fn default_rss_vec() -> Vec<String> {
    vec!["/blog/*".to_string()]
}

fn default_assets() -> Vec<String> {
    vec!["assets/**/*".to_string()]
}

fn default_empty_vec<T>() -> Vec<T> {
    vec![]
}

fn default_current_date() -> DateTime<Utc> {
    chrono::offset::Utc::now()
}
