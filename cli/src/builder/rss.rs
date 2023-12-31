use std::{fs, path::PathBuf};

use super::package_schema::NodeAuthor;
use chrono::{DateTime, Utc};
use rss::{
    Category, CategoryBuilder, ChannelBuilder, EnclosureBuilder, ImageBuilder, Item, ItemBuilder,
};
use serde_derive::Serialize;
use sqlx::{postgres::Postgres, Pool};

#[derive(Debug, Serialize, Clone, sqlx::Type)]
#[sqlx(type_name = "_author")]
struct Authors(Vec<NodeAuthor>);

#[derive(Debug, Serialize, Clone, sqlx::FromRow)]
struct SQLPostRss {
    id: i32,
    permalink: String,
    title: String,
    authors: Authors,
    description: String,
    keywords: Vec<String>,
    rss: Vec<String>,
    cover: String,
    date_published: DateTime<Utc>,
    root_path: String,
    output_path: String,
}

/// Build an RSS document based on the root permalink's publicly exposed RSS subdirectories.
pub async fn build_rss(pool: Pool<Postgres>) {
    let root_foil_permalink = "/".to_string();
    let found_root_post: Result<SQLPostRss, sqlx::Error> = sqlx::query_as(
        "SELECT id, permalink, title, authors, description, keywords, rss, cover, date_published, root_path, output_path FROM posts WHERE permalink = $1",
    )
    .bind(&root_foil_permalink)
    .fetch_one(&pool)
    .await;
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
            let image = ImageBuilder::default()
                .url(root_post.cover)
                .link(author.url.clone())
                .title(root_post.title.clone())
                .width(Some("1920".to_string()))
                .height(Some("1080".to_string()))
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
            let found_items: Vec<SQLPostRss> = sqlx::query_as(
                "SELECT id, permalink, title, authors, description, keywords, rss, cover, date_published, root_path, output_path FROM posts WHERE permalink LIKE '/blog/%'",
            )
            .fetch_all(&pool)
            .await.unwrap_or(vec![]);
            for found_item in found_items {
                let byte_length = "1024".to_string();
                let enclosure = EnclosureBuilder::default()
                    .url(found_item.cover.clone())
                    .length(byte_length)
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
            let rss_out_path = PathBuf::new().join(&root_post.root_path).join(&root_post.output_path).join("rss.xml");
            let write_result = fs::write(rss_out_path, channel.to_string());
            if write_result.is_err() {
                println!("❌ Failed to write RSS output.");
                return;
            }
        }
    }
}
