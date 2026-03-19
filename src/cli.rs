use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::prelude::*;
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use axum::{
    routing::{get, post},
    Router,
};
use crate::api::{
    get_health, upload_object, get_object_by_cid, head_object_by_cid,
    get_metrics, link_name, resolve_name, unlink_name,
};
use crate::{Metadata, Storage, AppState};

#[derive(Parser)]
#[command(name = "s3lite")]
#[command(about = "S3-Lite CLI - Content-Addressed Storage", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the S3-Lite server
    Server {
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    /// Upload a file
    Upload {
        /// Path to the file to upload
        file: String,
        
        /// Optional name for the file
        #[arg(long)]
        name: Option<String>,
    },
    /// Download a file by CID
    Download {
        /// Content ID (CID) of the file
        cid: String,
        
        /// Output file path
        #[arg(long)]
        output: String,
    },
    /// Show server metrics
    Metrics,
}

 async fn handle_server(port: u16) -> anyhow::Result<()> {
    use crate::{storage::Storage, metadata::Metadata};
    use axum::{
        routing::{get, post},
        Router,
    };
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use std::net::SocketAddr;

    // Logs: export RUST_LOG=info for verbosity
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "s3lite=info,tower_http=info".into()))
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
    let listener = TcpListener::bind(addr).await?;
    info!("S3-Lite listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_upload(file: String, name: Option<String>) -> anyhow::Result<()> {
    // TODO: Read file, make HTTP POST request
    Ok(())
}

async fn handle_download(cid: String, output: String) -> anyhow::Result<()> {
    // TODO: Make HTTP GET request, write to file
    Ok(())
}

async fn handle_metrics() -> anyhow::Result<()> {
    // TODO: Make HTTP GET request to /metrics
    Ok(())
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Server { port } => handle_server(port).await,
        Commands::Upload { file, name } => handle_upload(file, name).await,
        Commands::Download { cid, output } => handle_download(cid, output).await,
        Commands::Metrics => handle_metrics().await,
    }
}