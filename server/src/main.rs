use std::error::Error;

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use iowebsite_backend::{build_router, config::Config, state::AppState, upstream::UpstreamClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("iowebsite=info,tower_http=info")),
        )
        .init();

    let config = Config::from_env()?;
    let port = config.port;
    let static_dir = config.static_dir.clone();
    let upstream = UpstreamClient::new()?;
    let state = AppState::new(upstream);
    let app = build_router(state, static_dir.clone());
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;

    tracing::info!(
        port,
        version = iowebsite_backend::APP_VERSION,
        static_dir = %static_dir.display(),
        "iowebsite server listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
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
