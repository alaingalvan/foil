use crate::clean_path_string;
use async_graphql::dataloader::Loader;
use async_graphql::futures_util::TryStreamExt;
use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use log::error;
use serde::Serialize;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn err<'a, T>(msg: &'a str) -> Result<T, async_graphql::Error> {
    Err(async_graphql::Error::new(msg))
}

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
    pub covers: Vec<String>,
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
    pub covers: Vec<String>,
    /// Main javascript file for this post with a default react component export.
    pub main: String,
    /// ⏰ The time this post was published.
    pub date_published: DateTime<Utc>,
    /// ⏱️ The time this post was updated.
    pub date_modified: DateTime<Utc>,
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
                        covers: sql_post.1.covers,
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
    /// 📚 Find all posts that correspond to a given list of permalinks.
    async fn posts_from_permalinks(
        &self,
        ctx: &Context<'_>,
        permalinks: Vec<String>,
    ) -> Result<Vec<Post>> {
        if permalinks.len() > 10 {
            return err("Permalink list must be < 10!");
        }
        for permalink in permalinks.iter() {
            if permalink.len() > 254 {
                let err_str = format!(
                    "Permalinks must have less than 254 characters. Check {}.",
                    &permalink
                );
                return err(&err_str);
            }
        }
        let postgres_pool: &Pool<Postgres> = ctx.data_opt().unwrap();
        let cur_query = include_str!("sql/posts_from_permalinks.sql");
        let sql_result: Vec<Post> = sqlx::query_as(&cur_query)
            .bind(&permalinks)
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
                covers: sql_post.covers,
                main: sql_post.main,
                date_published: sql_post.date_published,
                date_modified: sql_post.date_modified,
            })
            .try_collect()
            .await
            .unwrap_or(vec![]);
        Ok(sql_result)
    }

    /// 🗞️ Find post(s) based off a given permalink, which can be in a unix glob pattern.
    async fn posts_from_glob(
        &self,
        ctx: &Context<'_>,
        permalink: String,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<Post>> {
        if permalink.len() > 254 {
            let err_str = format!(
                "Permalinks must have less than 254 characters. Check {}.",
                &permalink
            );
            return err(&err_str);
        }

        let postgres_pool: &Pool<Postgres> = ctx.data_opt().unwrap();
        let cur_query = include_str!("sql/posts_from_glob.sql");
        //Convert the following permalink to regex depending on if there's a star in it.
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

        let mut offset = offset.unwrap_or(0);
        if offset > 10000 {
            offset = 10000;
        }
        let mut limit = limit.unwrap_or(10);
        if limit > 100 {
            limit = 100;
        }

        let sql_result: Vec<Post> = sqlx::query_as(&cur_query)
            .bind(&permalink_regex)
            .bind(&limit)
            .bind(&offset)
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
                covers: sql_post.covers,
                main: sql_post.main,
                date_published: sql_post.date_published,
                date_modified: sql_post.date_modified,
            })
            .try_collect()
            .await
            .unwrap_or(vec![]);
        Ok(sql_result)
    }

    /// 🪺 Find the post closest to a given permalink by recursively querying down.
    async fn post_recursive(&self, ctx: &Context<'_>, permalink: String) -> Result<Option<Post>> {
        if permalink.len() > 254 {
            let err_str = format!(
                "Permalinks must have less than 254 characters. Check {}.",
                &permalink
            );
            return err(&err_str);
        }
        let postgres_pool: &Pool<Postgres> = ctx.data_opt().unwrap();
        let path_pathbuf = PathBuf::from(permalink);
        let mut path_ancestors = path_pathbuf.ancestors();
        loop {
            let ancestor = path_ancestors.next();
            match ancestor {
                Some(par) => {
                    let par_path_buf = par.to_path_buf();
                    let par_clean = clean_path_string(&par_path_buf);
                    let cur_query = include_str!("sql/post_recursive_public.sql");
                    let sql_result: Result<SQLPost, sqlx::Error> = sqlx::query_as(&cur_query)
                        .bind(&par_clean)
                        .fetch_one(postgres_pool)
                        .await;
                    match sql_result {
                        Ok(sql_post) => {
                            return Ok(Some(Post {
                                id: sql_post.id,
                                permalink: sql_post.permalink,
                                title: sql_post.title,
                                authors: sql_post.authors.0,
                                description: sql_post.description,
                                keywords: sql_post.keywords,
                                covers: sql_post.covers,
                                main: sql_post.main,
                                date_published: sql_post.date_published,
                                date_modified: sql_post.date_modified,
                            }));
                        }
                        Err(_sql_e) => (),
                    }
                }
                None => {
                    break;
                }
            }
        }
        return Ok(None);
    }

    /// 🔎🍎 Search posts based on a given search string.
    async fn post_search(&self, ctx: &Context<'_>, search_string: String) -> Result<Vec<Post>> {
        if search_string.len() > 254 {
            let err_str = format!(
                "Search string must have less than 254 characters. Check {}.",
                &search_string
            );
            return err(&err_str);
        }
        let postgres_pool: &Pool<Postgres> = ctx.data_opt().unwrap();
        let cur_query = include_str!("sql/post_search.sql");
        let sanitized_string = search_string.replace("%", "").replace("_", "");
        let str = "%".to_string() + &sanitized_string + "%";
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
                covers: sql_post.covers,
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
