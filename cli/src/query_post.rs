use crate::builder::package_schema::NodeAuthor;
use chrono::{DateTime, Utc};
use serde_derive::Serialize;
use sqlx::{postgres::Postgres, Pool};
#[derive(Debug, Serialize, Clone, sqlx::Type)]
#[sqlx(type_name = "_author")]
pub struct Authors(pub Vec<NodeAuthor>);

#[derive(Debug, Serialize, Clone, sqlx::FromRow)]
pub struct SQLPost {
    pub id: i32,
    pub name: String,
    pub permalink: String,
    pub title: String,
    pub authors: Authors,
    pub description: String,
    pub keywords: Vec<String>,
    pub rss: Vec<String>,
    pub covers: Vec<String>,
    pub date_published: DateTime<Utc>,
    pub root_path: String,
    pub output_path: String,
    pub public_modules: Vec<String>,
}

/// Build an RSS document based on the root permalink's publicly exposed RSS subdirectories.
pub async fn query_post(pool: &Pool<Postgres>, permalink: String) -> Result<SQLPost, sqlx::Error> {
    let found_root_post: Result<SQLPost, sqlx::Error> =
        sqlx::query_as(include_str!("sql/post_from_permalink.sql"))
            .bind(&permalink)
            .fetch_one(pool)
            .await;
    found_root_post
}

pub async fn query_posts(
    pool: &Pool<Postgres>,
    permalink: String,
) -> Result<Vec<SQLPost>, sqlx::Error> {
    let found_root_post: Result<Vec<SQLPost>, sqlx::Error> =
        sqlx::query_as(include_str!("sql/posts_from_glob.sql"))
            .bind(&permalink)
            .fetch_all(pool)
            .await;
    found_root_post
}
