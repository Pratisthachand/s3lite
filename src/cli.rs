use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::prelude::*;
use crate::api::{
    get_health, upload_object_stream, get_object_by_cid, head_object_by_cid,
    get_metrics, link_name, resolve_name, unlink_name, dashboard, delete_object
};
use crate::AppState;

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

 async fn handle_server(_port: u16) -> anyhow::Result<()> {
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
        .route("/objects", post(upload_object_stream))
        .route(
            "/objects/:cid", 
            get(get_object_by_cid)
                .head(head_object_by_cid)
                .delete(delete_object) 
        )
        .route("/links", post(link_name))
        .route("/links/:name", get(resolve_name))
        .route("/links/:name", axum::routing::delete(unlink_name))
        .route("/metrics", get(get_metrics))
        .route("/dashboard", get(dashboard)) 
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    info!("S3-Lite listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_upload(file: String, name: Option<String>) -> anyhow::Result<()> {
    // Step 1: Read file
    let bytes = tokio::fs::read(&file).await?;
    println!("Read {} bytes from {}", bytes.len(), file);
    
    // Step 2: Create HTTP client
    let client = reqwest::Client::new();
    
    // Step 3: Build URL with query param
    let mut url = String::from("http://localhost:8080/objects");
    if let Some(n) = &name {
        url.push_str(&format!("?name={}", n));
    }
    
    // Step 4: Make POST request
    let response = client
        .post(&url)
        .body(bytes)
        .send()
        .await?;
    
    // Step 5: Parse response JSON
    let json: serde_json::Value = response.json().await?;
    
    // Step 6: Print nicely
    println!("{}", serde_json::to_string_pretty(&json)?);
    
    Ok(())
}

async fn handle_download(cid: String, output: String) -> anyhow::Result<()> {
    // Create HTTP client
    let client = reqwest::Client::new();
    
    // Build the download URL
    let url = format!("http://localhost:8080/objects/{}", cid);
    
    // Send GET request to download the file
    let response = client.get(&url).send().await?;
    
    // Check if the request was successful (HTTP 200)
    if response.status() != 200 {
        anyhow::bail!("Failed to download: {}", response.status());
    }
    
    // Convert response to bytes
    let bytes = response.bytes().await?;
    
    // Write bytes to output file on disk
    tokio::fs::write(&output, bytes).await?;
    
    println!("Downloaded to {}", output);
    
    Ok(())
}

async fn handle_metrics() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:8080/metrics")
        .send()
        .await?;
    
    let json: serde_json::Value = response.json().await?;
    println!("{}", serde_json::to_string_pretty(&json)?);
    
    Ok(())
}

pub async fn run() -> anyhow::Result<()> {
    // Parse command-line arguments using clap
    let cli = Cli::parse();
    
    // Match on the command and call the appropriate handler
    match cli.command {
        Commands::Server { port } => handle_server(port).await,
        Commands::Upload { file, name } => handle_upload(file, name).await,
        Commands::Download { cid, output } => handle_download(cid, output).await,
        Commands::Metrics => handle_metrics().await,
    }
}