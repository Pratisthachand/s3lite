use axum::{
    routing::{get, post},
    Router, Json,
};
use std::{net::SocketAddr, sync::Arc};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::net::TcpListener;

mod storage;
mod metadata;
mod api;
mod errors;
mod cli;

use api::{
    get_health, upload_object_stream, get_object_by_cid, head_object_by_cid,
    get_metrics, link_name, resolve_name, unlink_name,
};
use metadata::Metadata;
use storage::Storage;

#[derive(Clone)]
pub struct AppState {
    storage: Arc<Storage>,
    meta: Arc<Metadata>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}