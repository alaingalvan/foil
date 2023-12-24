use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};
pub type StringMap = std::collections::HashMap<String, String>;

//=====================================================================================================================
/// 📦 A Foil node package.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NodePackage {
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
#[derive(Serialize, Deserialize, Clone, Debug)]
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

    /// React JavaScript/TypeScript file with HTML of page as default export.
    #[serde(default = "default_empty_str")]
    pub html: String,

    /// is this a Foil frontend? If so it's compiled with public modules exposed with SystemJS.
    #[serde(default = "default_false")]
    pub frontend: bool,

    /// Permalink redirects, used when renaming foil posts but you need to maintain the original URL.
    #[serde(default = "default_empty_vec")]
    pub redirects: Vec<FoilRedirect>,

    /// RSS glob path to export posts from.
    #[serde(default = "default_rss_str")]
    pub rss: String,
}

//=====================================================================================================================
/// File/modified date pair.
#[derive(Clone, Serialize, Deserialize, sqlx::Type, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoilRedirect {
    /// Path redirecting from.
    pub from: String,
    /// Path redirecting to.
    pub to: String,
}

//=====================================================================================================================

fn default_false() -> bool {
    false
}

fn default_empty_str() -> String {
    "".to_string()
}

fn default_rss_str() -> String {
    "/*".to_string()
}

fn default_assets() -> Vec<String> {
    vec!["assets/*".to_string()]
}

fn default_empty_vec<T>() -> Vec<T> {
    vec![]
}

fn default_current_date() -> DateTime<Utc> {
    chrono::offset::Utc::now()
}
