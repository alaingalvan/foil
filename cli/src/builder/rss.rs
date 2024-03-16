use super::package_schema::NodeAuthor;
use rss::{
    Category, CategoryBuilder, ChannelBuilder, EnclosureBuilder, ImageBuilder, Item, ItemBuilder,
};
use sqlx::{postgres::Postgres, Pool};
use std::{fs, path::PathBuf};

use crate::query_post::{query_post, query_posts, SQLPost};

fn get_cover_permalink_and_path(post: &SQLPost) -> (String, PathBuf) {
    let default_cover = "".to_string();
    let cover = post.covers.get(0).unwrap_or(&default_cover);
    let mut root_string = post.root_path.clone();
    if !(root_string.ends_with("/") || root_string.ends_with("\\")) {
        root_string += "/";
    }
    let cover_relative_permalink = cover.replacen(&post.permalink, "", 1);
    let cover_rel = if cover_relative_permalink.starts_with("/") {
        cover_relative_permalink.replacen("/", "", 1)
    } else {
        cover_relative_permalink.clone()
    };

    let root_path = PathBuf::from(root_string);
    let cover_rel_path = PathBuf::from(&cover_rel);
    let cover_path = root_path.join(cover_rel_path);
    (cover.clone(), cover_path.clone())
}

/// Build an RSS document based on the root permalink's publicly exposed RSS subdirectories.
pub async fn build_rss(pool: Pool<Postgres>) {
    let found_root_post: Result<SQLPost, sqlx::Error> = query_post(&pool, "/".to_string()).await;
    match found_root_post {
        Err(e) => {
            if cfg!(debug_assertions) {
                println!("{:?}", e);
            }
        }
        Ok(root_post) => {
            let default_author = NodeAuthor {
                name: "Foil".to_string(),
                email: "hi@foil.email".to_string(),
                url: "/".to_string(),
            };
            let author = root_post.authors.0.get(0).unwrap_or(&default_author);
            let mut categories: Vec<Category> = vec![];
            for tag in root_post.keywords.clone() {
                let cat = CategoryBuilder::default().name(tag).build();
                categories.push(cat);
            }
            let (cover, cover_server_path) = get_cover_permalink_and_path(&root_post);
            let (width, height) = match imagesize::size(&cover_server_path) {
                Ok(v) => (v.width, v.height),
                Err(_ie) => (0, 0),
            };
            let image = ImageBuilder::default()
                .url(cover)
                .link(author.url.clone())
                .title(root_post.title.clone())
                .width(Some(width.to_string()))
                .height(Some(height.to_string()))
                .description(Some(root_post.title.clone()))
                .build();
            let copyright = "Copyright ".to_string() + &author.name + " All Rights Reserved";

            // 🌳 Define our application RSS channel:
            let mut channel = ChannelBuilder::default()
                .title(root_post.title.clone())
                .link(author.url.clone())
                .description(root_post.description)
                .copyright(Some(copyright))
                .managing_editor(Some(author.name.clone()))
                .webmaster(Some(author.name.clone()))
                .categories(categories)
                .image(Some(image))
                .ttl(Some("1200".to_string()))
                .build();

            // 🥬 Build RSS Items:
            let mut items: Vec<Item> = vec![];
            let permalink = "/blog/*".to_string();
            let mut permalink_regex = "^".to_string();
            for char in permalink.chars() {
                let char_string = char.to_string();
                let char_str = &char_string;
                match char {
                    '/' | '$' | '^' | '+' | '.' | '(' | ')' | '=' | '!' | '|' | ',' | '{' | '}'
                    | '[' | ']' => {
                        permalink_regex += "\\";
                        permalink_regex += char_str;
                    }
                    '*' => {
                        permalink_regex += ".*";
                    }
                    _ => permalink_regex += char_str,
                }
            }
            permalink_regex += "$";
            let found_items: Vec<SQLPost> =
                query_posts(&pool, permalink_regex).await.unwrap_or(vec![]);
            for found_item in found_items {
                let (cover, cover_server_path) = get_cover_permalink_and_path(&found_item);
                let byte_length = match fs::metadata(&cover_server_path) {
                    Ok(meta) => {
                        let s = meta.len();
                        s
                    }
                    Err(_e) => 0,
                };
                let cover_ext = cover_server_path.extension().unwrap_or_default();
                let mime = "image/".to_string() + cover_ext.to_str().unwrap_or_default();
                let enclosure = EnclosureBuilder::default()
                    .url(cover)
                    .length(byte_length.to_string())
                    .mime_type(mime)
                    .build();
                let item_author = found_item.authors.0.get(0).unwrap_or(&default_author);
                let mut item_categories: Vec<Category> = vec![];
                for tag in found_item.keywords.clone() {
                    let cat = CategoryBuilder::default().name(tag).build();
                    item_categories.push(cat);
                }
                let item = ItemBuilder::default()
                    .title(Some(found_item.title))
                    .description(Some(found_item.description))
                    .link(Some(found_item.permalink))
                    .pub_date(Some(found_item.date_published.to_string()))
                    .categories(item_categories)
                    .author(Some(item_author.name.clone()))
                    .enclosure(Some(enclosure))
                    .build();
                items.push(item);
            }
            channel.set_items(items);

            // Write to output path:
            let rss_out_path = PathBuf::new()
                .join(&root_post.root_path)
                .join(&root_post.output_path)
                .join("rss.xml");
            let write_result = fs::write(&rss_out_path, channel.to_string());
            if write_result.is_err() {
                println!("❌ Failed to write RSS output.");
                return;
            } else {
                println!(
                    "🌊 Successfully generated RSS feed to {}.",
                    rss_out_path.to_str().unwrap_or_default()
                );
            }
        }
    }
}
