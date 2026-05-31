#[allow(dead_code)]
pub mod repo;

use sqlx::MySqlPool;
use tracing::info;

// Auto-generated at build time from src/db/migrations/*.sql (sorted by name).
// Adding a new .sql file is sufficient — no manual registration needed.
include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

pub async fn run_migrations(pool: &MySqlPool) -> anyhow::Result<()> {
    // Create tracking table
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name VARCHAR(255) PRIMARY KEY,
            applied_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3)
        )",
    )
    .execute(pool)
    .await?;

    for (name, sql) in MIGRATIONS {
        let already_applied: bool = sqlx::query("SELECT 1 FROM _migrations WHERE name = ?")
            .bind(name)
            .fetch_optional(pool)
            .await?
            .is_some();

        if already_applied {
            info!(migration = name, "migration already applied, skipping");
            continue;
        }

        info!(migration = name, "applying migration");
        sqlx::raw_sql(sql).execute(pool).await?;

        sqlx::query("INSERT INTO _migrations (name) VALUES (?)")
            .bind(name)
            .execute(pool)
            .await?;

        info!(migration = name, "migration applied successfully");
    }

    Ok(())
}
