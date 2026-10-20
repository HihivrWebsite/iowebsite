pub mod config;
pub mod normalize;
pub mod routes;
pub mod state;
pub mod upstream;

use std::path::{Path, PathBuf};

use axum::{routing::get, Router};
use tower_http::services::{ServeDir, ServeFile};

use crate::state::AppState;

pub const APP_VERSION: &str = include_str!("../../VERSION");

pub fn build_router(state: AppState, static_dir: PathBuf) -> Router {
    let index_path = static_dir.join("index.html");
    if !index_path.is_file() {
        tracing::warn!(
            path = %index_path.display(),
            "frontend index.html not found; API routes remain available"
        );
    }

    let static_service = ServeDir::new(&static_dir).fallback(ServeFile::new(index_path));

    Router::new()
        .route("/healthz", get(routes::health))
        .route("/api/version", get(routes::version))
        .route("/api/live-status", get(routes::live_status))
        .fallback_service(static_service)
        .with_state(state)
}

pub fn default_static_dir() -> PathBuf {
    let Ok(current_dir) = std::env::current_dir() else {
        return PathBuf::from("dist");
    };

    let candidates = [
        current_dir.join("dist"),
        current_dir.join("../dist").clean_join(),
    ];

    candidates
        .into_iter()
        .find(|path| Path::new(path).join("index.html").is_file())
        .unwrap_or_else(|| current_dir.join("dist"))
}

trait CleanJoin {
    fn clean_join(self) -> PathBuf;
}

impl CleanJoin for PathBuf {
    fn clean_join(self) -> PathBuf {
        let mut normalized = PathBuf::new();
        for component in self.components() {
            match component {
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::CurDir => {}
                other => normalized.push(other),
            }
        }
        normalized
    }
}
