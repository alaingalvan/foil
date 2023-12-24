use crate::error::Result;
use crate::misc::connect_db;
use sqlx::migrate::Migrator;
use std::path::Path;
pub async fn reset() -> Result<()> {
    // 📚 Configure Database:
    let postgres_pool = connect_db().await?;

    // Begin clear, relies on SQLX migrations:
    println!("🧪 Clearing Foil database...");
    let m = Migrator::new(Path::new("./migrations")).await?;
    match m.run(&postgres_pool).await {
        Ok(()) => {
            println!("🧑‍🔬 Reset foil database successfully.");
        }
        Err(e) => {
            println!("🫗 Reset database with errors.");
            println!("{:?}", e)
        }
    }

    match sqlx::query("DROP TABLE _sqlx_migrations")
        .execute(&postgres_pool)
        .await
    {
        Ok(_) => {}
        Err(e) => {
            println!("Dropped migrations failed:");
            println!("{:?}", e)
        }
    };
    Ok(())
}
