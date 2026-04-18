use std::sync::Arc;

mod storage;
mod metadata;
mod api;
mod errors;
mod cli;

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