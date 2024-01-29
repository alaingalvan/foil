use super::package_schema::NodeAuthor;
use super::read_foil_package;
use super::resolver::Foil;
use crate::error::Result;
use crate::return_err;
use chrono::{DateTime, Utc};
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

    // Define new datetimez:
    let dt = Utc::now();
    let naive_utc = dt.naive_utc();
    let offset = dt.offset().clone();
    let dt_new = DateTime::<Utc>::from_naive_utc_and_offset(naive_utc, offset);

    let found: (i32, DateTime<Utc>) =
        sqlx::query_as("SELECT id, date_modified FROM posts WHERE permalink = $1")
            .bind(&foil.permalink)
            .fetch_one(&pool)
            .await
            .unwrap_or((-1, dt_new));

    let post_id: i32 = found.0;
    let updating = post_id > 0;

    let authors_str = authors_as_sql(&foil.authors);
    let query = if !updating {
        format!(
            r#"
        INSERT INTO posts 
        (name, permalink, title, authors, description,
         keywords, covers, main, date_published,
         date_modified, output_path, root_path, public_modules,
         rss, assets) 
        VALUES ($1, $2, $3, ARRAY[{}]::author[], $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#,
            &authors_str
        )
    } else {
        format!(
            r#"UPDATE posts SET
        name = $1, title = $3, authors = ARRAY[{}]::author[], description = $4, 
        keywords = $5, covers = $6, main = $7, date_published = $8, 
        date_modified = $9, output_path = $10, root_path = $11, public_modules = $12, 
        rss = $13, assets = $14
        WHERE permalink = $2"#,
            &authors_str
        )
    };

    let resolved_main = foil.resolve_js_main();

    return_err!(
        sqlx::query(&query)
            .bind(&foil.name)
            .bind(&foil.permalink)
            .bind(&foil.title)
            .bind(&foil.description)
            .bind(&foil.keywords)
            .bind(&foil.covers)
            .bind(&resolved_main)
            .bind(&foil.date_published)
            .bind(&foil.date_modified)
            // Metadata
            .bind(&output_path_str)
            .bind(&root_path_str)
            .bind(&foil.public_modules)
            .bind(&foil.rss)
            .bind(&foil.assets)
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
/// 🧼 Clean the database of any stale/missing foil projects.
pub async fn clean_database(pool: Pool<Postgres>) -> Result<()> {
    // For each foil in the database, verify its corresponding output files exist.
    // This is a matter of first checking if its metadata exists, then verifying if its `package.json` exists.
    let clean_stream = sqlx::query("SELECT id, root_path, permalink FROM posts")
        .try_map(|row: PgRow| {
            Ok((
                row.try_get::<i32, _>(0).unwrap_or_default(),
                row.try_get::<String, _>(1).unwrap_or_default(),
                row.try_get::<String, _>(2).unwrap_or_default(),
            ))
        })
        .fetch(&pool)
        .map_ok(
            |(id, root_path, permalink)| match PathBuf::from_str(&root_path) {
                Ok(p) => {
                    let package_path = p.join("package.json");
                    let package_exists = package_path.exists();
                    let meta_exists = p.join("foil-meta.json").exists();
                    if !package_exists || !meta_exists {
                        id
                    } else {
                        match read_foil_package(&package_path) {
                            Ok(pack) => {
                                if pack.foil.permalink == permalink {
                                    -1
                                } else {
                                    id
                                }
                            }
                            Err(_e) => id,
                        }
                    }
                }
                _ => -1,
            },
        )
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
