use clap::{Parser, Subcommand};

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
    // TODO: Start server
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