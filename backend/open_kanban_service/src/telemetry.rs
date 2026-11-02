use anyhow::{Context, Result};
use tracing_subscriber::{EnvFilter, fmt};

/// Installs the global tracing subscriber once per process.
pub fn init(filter: &str, json: bool) -> Result<()> {
    let filter = EnvFilter::try_new(filter).context("invalid OK_LOG filter")?;

    let installed = if json {
        fmt().json().with_env_filter(filter).try_init()
    } else {
        fmt().with_env_filter(filter).try_init()
    };

    installed.map_err(|error| anyhow::anyhow!("failed to install tracing subscriber: {error}"))
}
