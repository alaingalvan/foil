use crate::error::Result;
use crate::misc::connect_db;
use sqlx::migrate::Migrator;
use std::path::Path;

pub async fn reset() -> Result<()> {
    // 📚 Configure Database:
    let postgres_pool = connect_db().await?;

    // 🧪 Run migrations
    println!("🧪 Clearing Foil database...");
    let migrator = Migrator::new(Path::new("./migrations")).await?;
    match migrator.run(&postgres_pool).await {
        Ok(()) => {
            println!("🧑‍🔬 Reset foil database successfully.");
        }
        Err(e) => {
            println!("🫗 Reset database with errors: {:?}", e);
            return Err(e.into());
        }
    }

    // 🗑️ Drop the migration table to force re-running of migrations next time
    match sqlx::query("DROP TABLE IF EXISTS _sqlx_migrations")
        .execute(&postgres_pool)
        .await
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Warning: Failed to drop migration table: {}", e);
        }
    };

    Ok(())
}

