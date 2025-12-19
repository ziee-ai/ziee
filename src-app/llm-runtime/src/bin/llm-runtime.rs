//! CLI tool for LLM Runtime - Standalone service for managing local LLM engines

use clap::{Parser, Subcommand};
use llm_runtime::{
    config::{DeviceType, EngineSettings, EngineType, GlobalSettings, InstanceConfig, RuntimeConfig},
    download::ModelDownloader,
    state::StateManager,
    EngineHandle, HealthStatus, Runtime,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "llm-runtime")]
#[command(about = "LLM Runtime - Standalone service for managing local LLM engines", long_about = None)]
#[command(version)]
struct Cli {
    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info", global = true)]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a model instance
    Start {
        /// Unique identifier for this instance
        instance_id: String,

        /// Engine to use (llamacpp or mistralrs)
        #[arg(short, long, value_parser = parse_engine_type)]
        engine: EngineType,

        /// Path to the model file
        #[arg(short, long)]
        model_path: PathBuf,

        /// Device to run on (cpu, cuda, metal, rocm, vulkan, opencl)
        #[arg(short, long, value_parser = parse_device_type, default_value = "cpu")]
        device: DeviceType,

        /// Engine-specific settings as JSON string
        /// Example for llamacpp: '{"ctx_size": 8192, "n_gpu_layers": 35}'
        /// Example for mistralrs: '{"max_seqs": 64, "dtype": "f16"}'
        /// Can also reference a file: @settings.json
        #[arg(short, long)]
        settings: Option<String>,

        /// Explicit port to bind to (optional, auto-assigned if not specified)
        #[arg(short, long)]
        port: Option<u16>,
    },

    /// Stop a running instance
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

    /// Show detailed information about an instance
    Info {
        /// Instance ID
        instance_id: String,
    },

    /// Download a model from HuggingFace
    Download {
        /// Repository ID (e.g., "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF")
        repo_id: String,

        /// File name to download (supports wildcards)
        #[arg(short, long)]
        file: String,

        /// Verify SHA256 checksum (optional)
        #[arg(long)]
        sha256: Option<String>,
    },

    /// Manage downloaded models
    #[command(subcommand)]
    Models(ModelsCommands),
}

#[derive(Subcommand)]
enum ModelsCommands {
    /// List all downloaded models
    List,

    /// Delete a downloaded model
    Delete {
        /// Repository ID
        repo_id: String,

        /// File name
        filename: String,
    },

    /// Show models directory location
    Dir,
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

    // Initialize state manager
    let state = StateManager::new()?;

    match cli.command {
        Commands::Start {
            instance_id,
            engine,
            model_path,
            device,
            settings,
            port,
        } => {
            // Check if model path exists
            if !model_path.exists() {
                eprintln!("✗ Model path does not exist: {}", model_path.display());
                std::process::exit(1);
            }

            // Parse settings
            let mut engine_settings = EngineSettings::default();

            if let Some(settings_str) = settings {
                // Handle @file.json syntax
                let json_str = if settings_str.starts_with('@') {
                    let file_path = settings_str.trim_start_matches('@');
                    std::fs::read_to_string(file_path)
                        .map_err(|e| format!("Failed to read settings file '{}': {}", file_path, e))?
                } else {
                    settings_str
                };

                // Parse JSON based on engine type
                match engine {
                    EngineType::Llamacpp => {
                        engine_settings.llamacpp = serde_json::from_str(&json_str)
                            .map_err(|e| format!("Failed to parse llamacpp settings: {}", e))?;
                    }
                    EngineType::Mistralrs => {
                        engine_settings.mistralrs = serde_json::from_str(&json_str)
                            .map_err(|e| format!("Failed to parse mistralrs settings: {}", e))?;
                    }
                }
            }

            // Set port if specified
            if let Some(p) = port {
                engine_settings.port = Some(p);
            }

            // Create instance config
            let instance_config = InstanceConfig {
                id: instance_id.clone(),
                engine,
                model_path,
                device,
                settings: engine_settings,
            };

            // Validate config
            if let Err(e) = instance_config.validate() {
                eprintln!("✗ Invalid configuration: {}", e);
                std::process::exit(1);
            }

            // Create minimal runtime config (we don't use YAML anymore)
            let runtime_config = RuntimeConfig {
                global: GlobalSettings::default(),
                instances: vec![instance_config.clone()],
            };

            println!("Starting instance: {}", instance_id);
            println!("  Engine:     {}", instance_config.engine);
            println!("  Model:      {}", instance_config.model_path.display());
            println!("  Device:     {}", instance_config.device);

            // Create runtime and start instance
            let mut runtime = Runtime::new(runtime_config).await?;
            let handle = runtime.start(&instance_id).await?;

            // Save to state
            state.save_instance(&instance_config, handle.pid, handle.port, &handle.base_url)?;

            print_handle(&handle);

            Ok(())
        }

        Commands::Stop { instance_id } => {
            // Get instance from state
            let instance_data = state.get_instance(&instance_id)?;

            if instance_data.is_none() {
                eprintln!("✗ Instance '{}' not found in state", instance_id);
                eprintln!("  Use 'llm-runtime list' to see running instances");
                std::process::exit(1);
            }

            let (instance_config, _pid, _port, _base_url) = instance_data.unwrap();

            // Create runtime with this instance
            let runtime_config = RuntimeConfig {
                global: GlobalSettings::default(),
                instances: vec![instance_config],
            };

            let mut runtime = Runtime::new(runtime_config).await?;

            println!("Stopping instance: {}", instance_id);
            runtime.stop(&instance_id).await?;

            // Remove from state
            state.delete_instance(&instance_id)?;

            println!("✓ Instance stopped");

            Ok(())
        }

        Commands::Health { instance_id } => {
            // Get instance from state
            let instance_data = state.get_instance(&instance_id)?;

            if instance_data.is_none() {
                eprintln!("✗ Instance '{}' not found in state", instance_id);
                std::process::exit(1);
            }

            let (instance_config, _pid, _port, _base_url) = instance_data.unwrap();

            // Create runtime
            let runtime_config = RuntimeConfig {
                global: GlobalSettings::default(),
                instances: vec![instance_config],
            };

            let runtime = Runtime::new(runtime_config).await?;

            println!("Checking health of: {}", instance_id);
            let health = runtime.health_check(&instance_id).await?;
            print_health(&instance_id, &health);

            match health {
                HealthStatus::Healthy => Ok(()),
                _ => std::process::exit(1),
            }
        }

        Commands::List => {
            let instances = state.list_instances()?;

            if instances.is_empty() {
                println!("No instances found");
                println!("\nStart an instance with:");
                println!("  llm-runtime start <id> --engine llamacpp --model-path /path/to/model.gguf");
            } else {
                println!("Stored instances ({}):", instances.len());
                println!();

                for (id, config) in instances {
                    println!("  {}", id);
                    println!("    Engine: {}", config.engine);
                    println!("    Model:  {}", config.model_path.display());
                    println!("    Device: {}", config.device);
                    println!();
                }
            }

            Ok(())
        }

        Commands::Info { instance_id } => {
            let instance_data = state.get_instance(&instance_id)?;

            if instance_data.is_none() {
                eprintln!("✗ Instance '{}' not found in state", instance_id);
                std::process::exit(1);
            }

            let (config, pid, port, base_url) = instance_data.unwrap();

            println!("Instance: {}", instance_id);
            println!("  Engine:     {}", config.engine);
            println!("  Model:      {}", config.model_path.display());
            println!("  Device:     {}", config.device);
            println!("  PID:        {}", pid);
            println!("  Port:       {}", port);
            println!("  Base URL:   {}", base_url);
            println!();
            println!("Settings:");

            match config.engine {
                EngineType::Llamacpp => {
                    let s = &config.settings.llamacpp;
                    println!("  Context size:   {}", s.ctx_size);
                    println!("  GPU layers:     {}", s.n_gpu_layers);
                    println!("  Batch size:     {}", s.batch_size);
                    if let Some(threads) = s.threads {
                        println!("  Threads:        {}", threads);
                    }
                    println!("  Embeddings:     {}", s.embeddings);
                }
                EngineType::Mistralrs => {
                    let s = &config.settings.mistralrs;
                    println!("  Max seqs:       {}", s.max_seqs);
                    println!("  Data type:      {}", s.dtype);
                    println!("  Model format:   {}", s.model_format);
                    println!("  Prefix cache:   {}", s.prefix_cache_n);
                    if let Some(mem) = s.pa_gpu_mem_mb {
                        println!("  GPU memory:     {}MB", mem);
                    }
                }
            }

            Ok(())
        }

        Commands::Download { repo_id, file, sha256 } => {
            let downloader = ModelDownloader::new()?;

            println!("Downloading model from HuggingFace");
            println!("  Repository: {}", repo_id);
            println!("  File:       {}", file);
            println!();

            let mut model_info = downloader.download(&repo_id, &file).await?;

            println!();
            println!("✓ Download complete:");
            println!("  Path: {}", model_info.path.display());
            println!("  Size: {}", llm_runtime::download::format_bytes(model_info.size_bytes));

            // Verify checksum if provided
            if let Some(expected_sha256) = sha256 {
                println!();
                println!("Verifying checksum...");
                let matches = downloader.verify_checksum(&mut model_info, &expected_sha256)?;

                if matches {
                    println!("✓ Checksum verified: {}", model_info.sha256.unwrap());
                } else {
                    eprintln!("✗ Checksum verification failed!");
                    std::process::exit(1);
                }
            }

            println!();
            println!("You can now use this model with:");
            println!("  llm-runtime start <id> --engine <type> --model-path {}", model_info.path.display());

            Ok(())
        }

        Commands::Models(models_cmd) => {
            let downloader = ModelDownloader::new()?;

            match models_cmd {
                ModelsCommands::List => {
                    let models = downloader.list_models()?;

                    if models.is_empty() {
                        println!("No downloaded models found");
                        println!("\nDownload a model with:");
                        println!("  llm-runtime download <repo-id> --file <filename>");
                    } else {
                        println!("Downloaded models ({}):", models.len());
                        println!();

                        for model in models {
                            println!("  {}", model.filename);
                            println!("    Repository: {}", model.repo_id);
                            println!("    Path:       {}", model.path.display());
                            println!("    Size:       {}", llm_runtime::download::format_bytes(model.size_bytes));
                            println!();
                        }
                    }

                    Ok(())
                }

                ModelsCommands::Delete { repo_id, filename } => {
                    downloader.delete_model(&repo_id, &filename)?;
                    println!("✓ Deleted model: {} / {}", repo_id, filename);
                    Ok(())
                }

                ModelsCommands::Dir => {
                    println!("Models directory: {}", downloader.models_dir().display());
                    Ok(())
                }
            }
        }
    }
}

fn parse_engine_type(s: &str) -> Result<EngineType, String> {
    match s.to_lowercase().as_str() {
        "llamacpp" | "llama" => Ok(EngineType::Llamacpp),
        "mistralrs" | "mistral" => Ok(EngineType::Mistralrs),
        _ => Err(format!(
            "Invalid engine type '{}'. Must be one of: llamacpp, mistralrs",
            s
        )),
    }
}

fn parse_device_type(s: &str) -> Result<DeviceType, String> {
    match s.to_lowercase().as_str() {
        "cpu" => Ok(DeviceType::Cpu),
        "cuda" => Ok(DeviceType::Cuda),
        "metal" => Ok(DeviceType::Metal),
        "rocm" => Ok(DeviceType::Rocm),
        "vulkan" => Ok(DeviceType::Vulkan),
        "opencl" => Ok(DeviceType::Opencl),
        _ => Err(format!(
            "Invalid device type '{}'. Must be one of: cpu, cuda, metal, rocm, vulkan, opencl",
            s
        )),
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