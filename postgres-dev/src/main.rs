mod config;
mod postgres;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "postgres-dev")]
#[command(about = "PostgreSQL development helper for ziee-chat", long_about = None)]
struct Args {
    /// Keep the PostgreSQL server alive after running migrations
    #[arg(long)]
    keep_alive: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("PostgreSQL Development Helper");
    println!("============================\n");

    // Load configuration
    let config = match config::Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            eprintln!("\nUsage:");
            eprintln!("  CONFIG_FILE=config/build.yaml cargo run [--keep-alive]");
            std::process::exit(1);
        }
    };

    if let Err(e) = postgres::setup_postgres(config, args.keep_alive).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
