use sqlx::{PgPool, Row};
use anyhow::Result;
use tracing::{info, error};

pub struct MigrationRunner {
    pool: PgPool,
}

impl MigrationRunner {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn run_migrations(&self) -> Result<()> {
        info!("Starting database migrations");

        // Create migrations table if it doesn't exist
        self.create_migrations_table().await?;

        // Run migrations in order
        let migrations = vec![
            ("001_comprehensive_schema", include_str!("../../sql/token_database_schema.sql")),
        ];

        for (name, sql) in migrations {
            if !self.is_migration_applied(name).await? {
                info!("Applying migration: {}", name);
                self.apply_migration(name, sql).await?;
            } else {
                info!("Migration {} already applied, skipping", name);
            }
        }

        info!("All migrations completed successfully");
        Ok(())
    }

    async fn create_migrations_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS migrations (
                id SERIAL PRIMARY KEY,
                name VARCHAR(255) NOT NULL UNIQUE,
                applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn is_migration_applied(&self, name: &str) -> Result<bool> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM migrations WHERE name = $1")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;

        let count: i64 = row.get("count");
        Ok(count > 0)
    }

    async fn apply_migration(&self, name: &str, sql: &str) -> Result<()> {
        // Execute the entire SQL as one statement to preserve transaction semantics
        match sqlx::raw_sql(sql).execute(&self.pool).await {
            Ok(_) => {
                info!("Migration SQL executed successfully");
            },
            Err(e) => {
                error!("Migration failed: {}", e);
                return Err(e.into());
            }
        }

        // Record the migration
        sqlx::query("INSERT INTO migrations (name) VALUES ($1)")
            .bind(name)
            .execute(&self.pool)
            .await?;

        info!("Successfully applied migration: {}", name);
        Ok(())
    }

    pub async fn get_applied_migrations(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT name FROM migrations ORDER BY applied_at")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().map(|row| row.get("name")).collect())
    }

    pub async fn rollback_migration(&self, name: &str) -> Result<()> {
        // This is a basic rollback - in production you'd want more sophisticated rollback logic
        sqlx::query("DELETE FROM migrations WHERE name = $1")
            .bind(name)
            .execute(&self.pool)
            .await?;

        info!("Rolled back migration: {}", name);
        Ok(())
    }
}
