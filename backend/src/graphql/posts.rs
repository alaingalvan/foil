use crate::graphql::err;
use async_graphql::dataloader::{DataLoader, Loader};
use async_graphql::futures_util::TryStreamExt;
use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use log::error;
use serde::Serialize;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;

/// 🎇 An author of a Foil post.
#[derive(Debug, Serialize, SimpleObject, Clone, sqlx::Type)]
#[sqlx(type_name = "author")]
pub struct Author {
    pub name: String,
    pub email: String,
    pub url: String,
}

#[derive(sqlx::Type, Clone)]
#[sqlx(type_name = "_author")]
pub struct SQLAuthors(pub Vec<Author>);

/// ✨ A Foil post schema for GraphQL.
#[derive(Debug, Serialize, SimpleObject, Clone)]
pub struct Post {
    /// 😎 ID for item.
    pub id: i32,
    /// 📎 Permalink (eg. https://<your-blog>/{permalink}).
    pub permalink: String,
    /// 👋 Name of this item.
    pub title: String,
    /// The author(s) of this post.
    pub authors: Vec<Author>,
    /// 📝 Short description (120-240 characters) about the post.
    pub description: String,
    /// 🔎 Search keywords for this post.
    pub keywords: Vec<String>,
    /// 📑 Cover image url for this post.
    pub cover: String,
    /// Main javascript file for this post with a default react component export.
    pub main: String,
    /// ⏰ The time this post was published.
    pub date_published: DateTime<Utc>,
    /// ⏱️ The time this post was updated.
    pub date_modified: DateTime<Utc>,
}

#[derive(sqlx::FromRow, Clone)]
pub struct SQLPost {
    /// 😎 ID for item.
    pub id: i32,
    /// 📎 Permalink (eg. https://<your-blog>/{permalink}).
    pub permalink: String,
    /// 👋 Name of this item.
    pub title: String,
    /// The author(s) of this post.
    pub authors: SQLAuthors,
    /// 📝 Short description (120-240 characters) about the post.
    pub description: String,
    /// 🔎 Search keywords for this post.
    pub keywords: Vec<String>,
    /// 📑 Cover image url for this post.
    pub cover: String,
    /// Main javascript file for this post with a default react component export.
    pub main: String,
    /// ⏰ The time this post was published.
    pub date_published: DateTime<Utc>,
    /// ⏱️ The time this post was updated.
    pub date_modified: DateTime<Utc>,
    /// Root folder where the foil post lives.
    pub root_path: String,
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct PostId(i32);
impl std::fmt::Display for PostId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct FoilLoader {
    pub pool: Pool<Postgres>,
}

impl FoilLoader {
    pub fn new(postgres_pool: Pool<Postgres>) -> Self {
        Self {
            pool: postgres_pool,
        }
    }
}

#[async_trait]
impl Loader<PostId> for FoilLoader {
    type Value = Post;
    type Error = FieldError;

    async fn load(&self, keys: &[PostId]) -> Result<HashMap<PostId, Self::Value>, Self::Error> {
        let sql_query = format!(include_str!("sql/post_load.sql"), keys.iter().join(","));
        let sql_postmap: HashMap<PostId, SQLPost> = sqlx::query_as(&sql_query)
            .fetch(&self.pool)
            .map_err(|x| {
                error!("Query Failed: {}", x.to_string());
            })
            .map_ok(|user: SQLPost| (PostId(user.id), user))
            .try_collect()
            .await
            .unwrap_or(HashMap::new());

        let mut m: HashMap<PostId, Self::Value> = HashMap::new();
        for sql_post in sql_postmap {
            let key = sql_post.0;

            if m.contains_key(&key) {
                continue;
            } else {
                m.insert(
                    key,
                    Post {
                        id: sql_post.1.id,
                        permalink: sql_post.1.permalink,
                        title: sql_post.1.title,
                        authors: sql_post.1.authors.0,
                        description: sql_post.1.description,
                        keywords: sql_post.1.keywords,
                        cover: sql_post.1.cover,
                        main: sql_post.1.main,
                        date_published: sql_post.1.date_published,
                        date_modified: sql_post.1.date_modified,
                    },
                );
            }
        }

        Ok(m)
    }
}

#[derive(Default)]
pub struct QueryPosts;

#[Object]
impl QueryPosts {
    /// 📄 Get details on a specific post based on its id.
    async fn post(&self, ctx: &Context<'_>, id: i32) -> Result<Option<Post>> {
        Ok(ctx
            .data_unchecked::<DataLoader<FoilLoader>>()
            .load_one(PostId(id))
            .await?)
    }

    /// 📰 Get details on multiple posts based on their id.
    async fn posts(&self, ctx: &Context<'_>, ids: Vec<i32>) -> Result<Vec<Post>> {
        Ok(ctx
            .data_unchecked::<DataLoader<FoilLoader>>()
            .load_many(ids.iter().map(|i| PostId(*i)))
            .await?
            .values()
            .cloned()
            .collect())
    }

    /// 🗞️ Find post based off a given permalink.
    async fn post_permalink(&self, ctx: &Context<'_>, permalink: String) -> Result<Option<Post>> {
        let postgres_pool: &Pool<Postgres> = ctx.data_opt().unwrap();
        let cur_query = include_str!("sql/post_permalink.sql");
        let sql_result: Result<SQLPost, sqlx::Error> = sqlx::query_as(&cur_query)
            .bind(&permalink)
            .fetch_one(postgres_pool)
            .await;
        if sql_result.is_err() {
            return err("Could not find foil post for the given permalink.");
        }
        let sql_post = sql_result.unwrap();

        Ok(Some(Post {
            id: sql_post.id,
            permalink: sql_post.permalink,
            title: sql_post.title,
            authors: sql_post.authors.0,
            description: sql_post.description,
            keywords: sql_post.keywords,
            cover: sql_post.cover,
            main: sql_post.main,
            date_published: sql_post.date_published,
            date_modified: sql_post.date_modified,
        }))
    }

    /// 📚 Get all posts with standard GraphQL connection pagination.
    async fn all_posts(
        &self,
        ctx: &Context<'_>,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<Post>> {
        let first = offset.unwrap_or(0);
        let mut last = limit.unwrap_or(10);
        if last > 100 {
            last = 100;
        }
        last += first;
        Ok(ctx
            .data_unchecked::<DataLoader<FoilLoader>>()
            .load_many((first..last).map(|i| PostId(i)))
            .await?
            .values()
            .cloned()
            .collect())
    }

    /// 🔎🍎 Search posts based on a given search string.
    async fn search_posts(&self, ctx: &Context<'_>, search_string: String) -> Result<Vec<Post>> {
        let postgres_pool: &Pool<Postgres> = ctx.data_opt().unwrap();
        let cur_query = include_str!("sql/post_search.sql");
        let str = "%".to_string() + &search_string + "%";
        let sql_result: Vec<Post> = sqlx::query_as(&cur_query)
            .bind(&str)
            .fetch(postgres_pool)
            .map_err(|x| {
                error!("Query failed: {}", x.to_string());
            })
            .map_ok(|sql_post: SQLPost| Post {
                id: sql_post.id,
                permalink: sql_post.permalink,
                title: sql_post.title,
                authors: sql_post.authors.0,
                description: sql_post.description,
                keywords: sql_post.keywords,
                cover: sql_post.cover,
                main: sql_post.main,
                date_published: sql_post.date_published,
                date_modified: sql_post.date_modified,
            })
            .try_collect()
            .await
            .unwrap_or(vec![]);
        Ok(sql_result)
    }
}
