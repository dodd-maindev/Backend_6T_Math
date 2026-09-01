use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use tokio::time::sleep;

/// Establishes an optimized high-concurrency connection pool to PostgreSQL.
pub async fn establish_connection(database_url: &str) -> PgPool {
    let mut attempts = 0;
    let max_attempts = 10;

    loop {
        match PgPoolOptions::new()
            .max_connections(50)
            .min_connections(5)
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .connect(database_url)
            .await
        {
            Ok(pool) => return pool,
            Err(err) => {
                attempts += 1;
                if attempts >= max_attempts {
                    panic!("Failed to connect to the database after {} attempts: {}", max_attempts, err);
                }
                println!("Database not ready, retrying in 3 seconds... (Attempt {}/{})", attempts, max_attempts);
                sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

/// Runs pending SQL database migrations.
pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("Failed to run database migrations");
}
