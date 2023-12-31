use super::package_schema::{FoilRedirect, NodeAuthor};
use super::resolver::Foil;
use crate::error::Result;
use crate::return_err;
use futures::{future, StreamExt, TryStreamExt};
use lexiclean::Lexiclean;
use path_slash::PathBufExt;
use sqlx::Row;
use sqlx::{postgres::PgRow, Pool, Postgres};
use std::path::PathBuf;
use std::str::FromStr;

//=====================================================================================================================
/// Update the database with a given foil post.
pub async fn update_foils(foil: &Foil, root_path: &PathBuf, pool: Pool<Postgres>) -> Result<()> {
    let root_path_str = root_path
        .clone()
        .lexiclean()
        .to_slash()
        .unwrap()
        .to_string()
        .replace("\\", "/");
    let output_path_str = foil
        .output_path
        .clone()
        .lexiclean()
        .to_slash()
        .unwrap()
        .to_string()
        .replace("\\", "/");

    let found: (i32, chrono::NaiveDateTime) =
        sqlx::query_as("SELECT id, date_modified FROM posts WHERE permalink = $1")
            .bind(&foil.permalink)
            .fetch_one(&pool)
            .await
            .unwrap_or((
                -1,
                chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap_or_default(),
            ));

    let post_id: i32 = found.0;
    let updating = post_id > 0;

    let authors_str = authors_as_sql(&foil.authors);
    let redirects_str = redirects_as_sql(&foil.redirects);
    let query = if !updating {
        format!(
            r#"
        INSERT INTO posts 
        (permalink, title, authors, description,
         keywords, cover, main, date_published,
         date_modified, output_path, root_path, public_modules,
         rss, redirects) 
        VALUES ($1, $2, ARRAY[{}]::author[], $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, ARRAY[{}]::redirect[])"#,
            &authors_str, &redirects_str
        )
    } else {
        format!(
            r#"UPDATE posts SET
        title = $2, authors = ARRAY[{}]::author[], description = $3, 
        keywords = $4, cover = $5, main = $6, date_published = $7, 
        date_modified = $8, output_path = $9, root_path = $10, public_modules = $11, 
        rss = $12, redirects = ARRAY[{}]::redirect[]
        WHERE permalink = $1"#,
            &authors_str, &redirects_str
        )
    };

    let resolved_main = foil.resolve_js_main();

    return_err!(
        sqlx::query(&query)
            .bind(&foil.permalink)
            .bind(&foil.title)
            .bind(&foil.description)
            .bind(&foil.keywords)
            .bind(&foil.cover)
            .bind(&resolved_main)
            .bind(&foil.date_published)
            .bind(&foil.date_modified)
            // Metadata
            .bind(&output_path_str)
            .bind(&root_path_str)
            .bind(&foil.public_modules)
            .bind(&foil.rss)
            .execute(&pool)
            .await,
        "Failed to insert foil post to database."
    );

    for asset in foil.assets.iter() {
        let found: (i32, String) =
            sqlx::query_as("SELECT id, permalink FROM assets WHERE permalink = $1")
                .bind(&asset)
                .fetch_one(&pool)
                .await
                .unwrap_or((-1, "/".to_string()));

        let db_id: i32 = found.0;
        let updating = db_id > 0;

        let query = {
            if !updating {
                "INSERT INTO assets (permalink, path) VALUES ($1, $2)"
            } else {
                "UPDATE assets SET path = $2 WHERE permalink = $1"
            }
        };

        let mut clean_asset = asset.clone();
        if clean_asset.ends_with("*") {
            clean_asset.remove(asset.len() - 1);
        }
        let cur_path: PathBuf = PathBuf::from(
            root_path
                .clone()
                .join(clean_asset)
                .lexiclean()
                .to_slash()
                .unwrap()
                .to_string(),
        );
        let p: String = cur_path
            .to_str()
            .unwrap_or("/")
            .to_string()
            .replace("\\", "/");

        return_err!(
            sqlx::query(&query)
                .bind(&asset)
                .bind(&p)
                .execute(&pool)
                .await,
            "Failed to insert foil post to database."
        );
    }

    Ok(())
}

//=====================================================================================================================
/// Write a node author structure as SQL.
fn authors_as_sql(authors: &Vec<NodeAuthor>) -> String {
    let mut out_str = "".to_string();
    for (i, author) in authors.iter().enumerate() {
        out_str += &format!(
            "('{}', '{}', '{}')",
            &author.name, &author.email, &author.url
        );
        if i != authors.len() - 1 {
            out_str += ","
        }
    }
    return out_str;
}

//=====================================================================================================================
/// Write a node author structure as SQL.
fn redirects_as_sql(redirects: &Vec<FoilRedirect>) -> String {
    let mut out_str = "".to_string();
    for (i, redirect) in redirects.iter().enumerate() {
        out_str += &format!("('{}', '{}')", &redirect.to, &redirect.from);
        if i != redirects.len() - 1 {
            out_str += ","
        }
    }
    return out_str;
}

//=====================================================================================================================
/// 🧼 Clean the database of any stale/missing foil projects.
pub async fn clean_database(pool: Pool<Postgres>) -> Result<()> {
    // For each foil in the database, verify its corresponding output files exist.
    // This is a matter of first checking if its metadata exists, then verifying if its `package.json` exists.
    let clean_stream = sqlx::query("SELECT id, root_path FROM posts")
        .try_map(|row: PgRow| {
            Ok((
                row.try_get::<i32, _>(0).unwrap_or_default(),
                row.try_get::<String, _>(1).unwrap_or_default(),
            ))
        })
        .fetch(&pool)
        .map_ok(|(id, root_path)| match PathBuf::from_str(&root_path) {
            Ok(p) => {
                let package_exists = p.join("package.json").exists();
                let meta_exists = p.join("foil-meta.json").exists();
                if !package_exists || !meta_exists {
                    id
                } else {
                    -1
                }
            }
            _ => -1,
        })
        .filter(|x| future::ready(x.is_ok() && *x.as_ref().unwrap() != -1));
    let clean_ids = clean_stream.collect::<Vec<_>>().await;
    for clean_id_result in clean_ids {
        match clean_id_result {
            Ok(id) => {
                let _q = sqlx::query("DELETE FROM posts WHERE id = $1")
                    .bind(id)
                    .execute(&pool)
                    .await?;
            }
            _ => (),
        }
    }

    Ok(())
}
