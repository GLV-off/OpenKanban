use std::{path::Path, str::FromStr, time::Duration};

use anyhow::{Context, Result};
use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

use crate::config::Config;

/// Migrations are embedded at compile time; applied on startup.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Builds connect options from a `sqlite:` URL.
pub fn connect_options(url: &str) -> Result<SqliteConnectOptions> {
    let options = SqliteConnectOptions::from_str(url)
        .with_context(|| format!("OK_DATABASE_URL is not a valid sqlite URL: `{url}`"))?;
    Ok(tune(options))
}

/// Builds connect options for a filesystem path (used by tests).
pub fn path_options(path: impl AsRef<Path>) -> SqliteConnectOptions {
    tune(SqliteConnectOptions::new().filename(path))
}

fn tune(options: SqliteConnectOptions) -> SqliteConnectOptions {
    options
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
}

pub async fn connect(config: &Config) -> Result<SqlitePool> {
    let options = connect_options(&config.database_url)?;
    open(options).await
}

pub async fn open(options: SqliteConnectOptions) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await
        .context("cannot open sqlite database")?;

    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    MIGRATOR
        .run(pool)
        .await
        .context("database migrations failed")?;
    Ok(())
}
