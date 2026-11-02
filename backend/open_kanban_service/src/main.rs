use anyhow::Context;
use open_kanban_service::{app, config::Config, db, state::AppState, telemetry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Convenience for local development; process environment always wins.
    let _ = dotenvy::dotenv();

    let config = Config::from_env()?;
    telemetry::init(&config.log_filter, config.environment.is_production())?;

    tracing::info!(
        bind = %config.bind,
        database = %config.database_url,
        environment = ?config.environment,
        "starting open_kanban_service"
    );

    let pool = db::connect(&config).await?;

    let bind = config.bind;
    let state = AppState::new(pool.clone(), config);
    let app = app::router(state);

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("cannot bind {bind}"))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("http server failed")?;

    pool.close().await;
    tracing::info!("stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to listen for ctrl-c");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
