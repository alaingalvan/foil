use crate::error::Result;
use sqlx::{Pool, Postgres};
use std::env;
use std::thread;

/// async-std's default task stack is 1 MiB, which is smaller than one sqlx 0.9
/// postgres query future. Callers that poll sqlx run on this larger stack.
const SQL_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Poll `work` on a thread whose stack can hold a sqlx 0.9 query future.
/// `async_std::task::spawn` still uses the 1 MiB worker stack, so this must be
/// a dedicated thread with its own runtime.
pub fn run_on_sql_stack<T, F>(name: &str, work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let handle = thread::Builder::new()
        .name(name.to_string())
        .stack_size(SQL_STACK_SIZE)
        .spawn(work)
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
    handle
        .join()
        .unwrap_or_else(|_| Err(format!("{name} thread panicked").into()))
}

/// The Foil database URL environment variable name.
pub const DATABASE_URL: &'static str = "FOIL_DATABASE_URL";

/// 🌐 Get the database URL from the runtime environment variable.
pub fn get_db_url() -> String {
    let db_url = match env::var(DATABASE_URL) {
        Ok(val) => val,
        Err(_e) => {
            println!("Couldn't find Foil database URL environment variable FOIL_DATABASE_URL.\nDefaulting to postgres://localhost/foil");
            "postgres://localhost/foil".to_string()
        }
    };
    db_url
}

/// 📚 Connect to the database for Foil.
pub async fn connect_db() -> Result<Pool<Postgres>> {
    // 📚 Configure Database:
    let db_url = get_db_url();
    let db_url_public_split: Vec<&str> = db_url.split("?").collect();
    let db_url_public = if db_url_public_split.len() >= 2 {
        db_url_public_split[0]
    } else {
        &db_url
    };
    println!("🐘 Opening PostgreSQL connection in: {}", db_url_public);
    let postgres_pool: Pool<Postgres> = match Pool::connect(&db_url).await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Failed to connect to database, is PostgreSQL running?");
            return Err(e.into());
        }
    };
    return Ok(postgres_pool);
}
