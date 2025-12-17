//! CLI tool for LLM Runtime

use clap::{Parser, Subcommand};
use llm_runtime::{EngineHandle, HealthStatus, InstanceInfo, Runtime, RuntimeConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "llm-runtime")]
#[command(about = "LLM Runtime - Manage local inference engines", long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE", default_value = "config.yaml")]
    config: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start an engine instance
    Start {
        /// Instance ID from configuration
        instance_id: String,
    },

    /// Stop an engine instance
    Stop {
        /// Instance ID
        instance_id: String,
    },

    /// Check health of an instance
    Health {
        /// Instance ID
        instance_id: String,
    },

    /// List all running instances
    List,

    /// Start all configured instances
    StartAll,

    /// Stop all running instances
    StopAll,

    /// Validate configuration file
    Validate,

    /// Run as daemon (start configured instances and keep running)
    Daemon,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = match cli.log_level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    // Load configuration
    let config = RuntimeConfig::from_file(&cli.config)?;

    match cli.command {
        Commands::Validate => {
            println!("✓ Configuration is valid");
            println!("  Global settings:");
            println!("    Log dir: {}", config.global.log_dir.display());
            println!("    Health check interval: {}s", config.global.health_check_interval_secs);
            println!("    Auto-restart: {}", config.global.auto_restart);
            println!("\n  Instances:");
            for instance in &config.instances {
                println!("    - {} ({})", instance.id, instance.engine);
            }
            Ok(())
        }

        Commands::Start { instance_id } => {
            let mut runtime = Runtime::new(config).await?;
            println!("Starting instance: {}", instance_id);

            let handle = runtime.start(&instance_id).await?;
            print_handle(&handle);

            Ok(())
        }

        Commands::Stop { instance_id } => {
            let mut runtime = Runtime::new(config).await?;
            println!("Stopping instance: {}", instance_id);

            runtime.stop(&instance_id).await?;
            println!("✓ Instance stopped");

            Ok(())
        }

        Commands::Health { instance_id } => {
            let runtime = Runtime::new(config).await?;
            println!("Checking health of: {}", instance_id);

            let health = runtime.health_check(&instance_id).await?;
            print_health(&instance_id, &health);

            match health {
                HealthStatus::Healthy => Ok(()),
                _ => std::process::exit(1),
            }
        }

        Commands::List => {
            let runtime = Runtime::new(config).await?;
            let instances = runtime.list_instances().await;

            if instances.is_empty() {
                println!("No running instances");
            } else {
                println!("Running instances:");
                println!();
                for instance in instances {
                    print_instance(&instance);
                    println!();
                }
            }

            Ok(())
        }

        Commands::StartAll => {
            let mut runtime = Runtime::new(config).await?;
            let instance_ids: Vec<String> = runtime.config.instances.iter().map(|i| i.id.clone()).collect();

            println!("Starting {} instance(s)...", instance_ids.len());

            for instance_id in instance_ids {
                match runtime.start(&instance_id).await {
                    Ok(handle) => {
                        println!("✓ Started: {} at {}", instance_id, handle.base_url);
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to start {}: {}", instance_id, e);
                    }
                }
            }

            Ok(())
        }

        Commands::StopAll => {
            let mut runtime = Runtime::new(config).await?;
            println!("Stopping all instances...");

            runtime.shutdown().await?;
            println!("✓ All instances stopped");

            Ok(())
        }

        Commands::Daemon => {
            let mut runtime = Runtime::new(config).await?;

            // Start all instances
            let instance_ids: Vec<String> = runtime.config.instances.iter().map(|i| i.id.clone()).collect();

            println!("Starting daemon with {} instance(s)...", instance_ids.len());

            for instance_id in instance_ids {
                match runtime.start(&instance_id).await {
                    Ok(handle) => {
                        println!("✓ Started: {} at {}", instance_id, handle.base_url);
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to start {}: {}", instance_id, e);
                    }
                }
            }

            // Setup signal handler
            println!("\nDaemon running. Press Ctrl+C to shutdown.");

            let ctrl_c = tokio::signal::ctrl_c();
            tokio::pin!(ctrl_c);

            ctrl_c.await?;

            println!("\nShutting down...");
            runtime.shutdown().await?;
            println!("✓ Shutdown complete");

            Ok(())
        }
    }
}

fn print_handle(handle: &EngineHandle) {
    println!("✓ Instance started:");
    println!("  ID:       {}", handle.instance_id);
    println!("  PID:      {}", handle.pid);
    println!("  Port:     {}", handle.port);
    println!("  Base URL: {}", handle.base_url);
}

fn print_health(instance_id: &str, health: &HealthStatus) {
    match health {
        HealthStatus::Healthy => {
            println!("✓ {} is healthy", instance_id);
        }
        HealthStatus::Starting => {
            println!("⋯ {} is starting", instance_id);
        }
        HealthStatus::Unhealthy(reason) => {
            println!("✗ {} is unhealthy: {}", instance_id, reason);
        }
        HealthStatus::Crashed => {
            println!("✗ {} has crashed", instance_id);
        }
    }
}

fn print_instance(instance: &InstanceInfo) {
    let health_symbol = match instance.health {
        HealthStatus::Healthy => "✓",
        HealthStatus::Starting => "⋯",
        HealthStatus::Unhealthy(_) => "✗",
        HealthStatus::Crashed => "✗",
    };

    println!("{} {}", health_symbol, instance.id);
    println!("  URL:     {}", instance.base_url);
    println!("  PID:     {}", instance.pid);
    println!("  Uptime:  {}s", instance.uptime_secs);
    println!("  Restarts: {}", instance.restart_count);

    if let HealthStatus::Unhealthy(reason) = &instance.health {
        println!("  Status:  Unhealthy ({})", reason);
    } else {
        println!("  Status:  {:?}", instance.health);
    }
}
