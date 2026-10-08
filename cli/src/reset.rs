use crate::error::Result;
use crate::misc::connect_db;
use sqlx::{Pool, Postgres};
use std::fs;
use std::path::PathBuf;

const FOIL_META_FILE: &str = "foil-meta.json";

/// Delete `foil-meta.json` for every post about to be removed from the database.
///
/// Roots come from `posts.root_path`, the same path a build writes metadata to.
/// A missing file is fine: the post may never have been built.
async fn clear_post_metadata(pool: &Pool<Postgres>) -> Result<()> {
    let roots: Vec<String> = match sqlx::query_scalar("SELECT root_path FROM posts")
        .fetch_all(pool)
        .await
    {
        Ok(roots) => roots,
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("42P01") => {
            println!("No posts table found, skipping metadata cleanup.");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    if roots.is_empty() {
        println!("No posts found, skipping metadata cleanup.");
        return Ok(());
    }

    println!("🗑️  Clearing metadata for {} post(s)...", roots.len());
    for root in roots {
        let meta_path = PathBuf::from(&root).join(FOIL_META_FILE);
        match fs::remove_file(&meta_path) {
            Ok(()) => println!("🗑️  Removed {}", meta_path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                eprintln!("Warning: Failed to remove {}: {}", meta_path.display(), e);
            }
        }
    }

    Ok(())
}

/// Drop and recreate the post cache by running the embedded reset SQL directly.
async fn reset_database(pool: &Pool<Postgres>) -> Result<()> {
    println!("🧪 Clearing Foil database...");
    sqlx::raw_sql(include_str!("sql/reset.sql"))
        .execute(pool)
        .await?;
    println!("🧑‍🔬 Reset foil database successfully.");
    Ok(())
}

pub async fn reset() -> Result<()> {
    let postgres_pool = connect_db().await?;
    clear_post_metadata(&postgres_pool).await?;
    reset_database(&postgres_pool).await?;
    Ok(())
}

