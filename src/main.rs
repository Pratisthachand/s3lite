use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse},
    routing::{get, head, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod storage;
mod metadata;
mod api;
mod errors;

use api::{upload_object, get_object_by_cid, head_object_by_cid, get_metrics, link_name, resolve_name, unlink_name, get_health};
use metadata::Metadata;
use storage::Storage;

#[derive(Clone)]
struct AppState {
    storage: Arc<Storage>,
    meta: Arc<Metadata>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logs: export RUST_LOG=info for verbosity
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "s3lite=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Config via ENV (defaults for speed)
    let data_dir = std::env::var("S3LITE_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8080);

    let storage = Arc::new(Storage::new(data_dir)?);
    let meta = Arc::new(Metadata::open("./meta.sled")?);

    let state = AppState { storage, meta };

    let app = Router::new()
        .route("/health", get(get_health))
        .route("/objects", post(upload_object))
        .route("/objects/:cid", get(get_object_by_cid).head(head_object_by_cid))
        .route("/links", post(link_name))
        .route("/links/:name", get(resolve_name))
        .route("/links/:name", axum::routing::delete(unlink_name))
        .route("/metrics", get(get_metrics))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("S3-Lite listening on http://{addr}");
    axum::Server::bind(&addr).serve(app.into_make_service()).await?;
    Ok(())
}