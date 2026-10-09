use crate::error::Result;
use crate::misc::connect_db;
use async_std::task;
use sqlx::{Pool, Postgres};
use std::fs;
use std::path::PathBuf;
use std::thread;

const FOIL_META_FILE: &str = "foil-meta.json";
/// Postgres `undefined_table`. The cache may not exist yet on a fresh database.
const UNDEFINED_TABLE: &str = "42P01";
/// async-std's default task stack is 1 MiB, which is smaller than one sqlx 0.9
/// postgres query future. Reset runs on its own thread so that future fits.
const RESET_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Delete `foil-meta.json` for every post about to be removed from the database.
async fn clear_post_metadata(pool: &Pool<Postgres>) -> Result<()> {
    let fetched = sqlx::query_scalar("SELECT root_path FROM posts")
        .fetch_all(pool)
        .await;
    let roots: Vec<String> = match fetched {
        Ok(roots) => roots,
        Err(e)
            if e.as_database_error()
                .and_then(|db| db.code())
                .as_deref()
                == Some(UNDEFINED_TABLE) =>
        {
            println!("No posts table found, skipping metadata cleanup.");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    if roots.is_empty() {
        println!("No posts found, skipping metadata cleanup.");
        return Ok(());
    }

    println!("🗑️ Clearing metadata for {} post(s)...", roots.len());
    for root in roots {
        let meta_path = PathBuf::from(&root).join(FOIL_META_FILE);
        match fs::remove_file(&meta_path) {
            Ok(()) => println!("🗑️ Removed {}", meta_path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
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

async fn reset_on_thread() -> Result<()> {
    let postgres_pool = connect_db().await?;
    clear_post_metadata(&postgres_pool).await?;
    reset_database(&postgres_pool).await?;
    Ok(())
}

pub async fn reset() -> Result<()> {
    // `spawn` still polls on async-std's 1 MiB worker stacks. A dedicated thread
    // gets its own runtime, so the sqlx future is polled on this larger stack.
    let handle = thread::Builder::new()
        .name("foil-reset".to_string())
        .stack_size(RESET_STACK_SIZE)
        .spawn(|| task::block_on(reset_on_thread()))
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    handle
        .join()
        .unwrap_or_else(|_| Err("foil reset thread panicked".into()))
}

