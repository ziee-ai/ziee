use std::fs;
use std::process::Command;
use std::thread;

use crate::config::Config;

/// Setup PostgreSQL server for development
/// This starts a PostgreSQL server for the application to use during development
pub async fn setup_postgres(config: Config, keep_alive: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let postgresql_version = &config.postgresql.version;
    println!("Starting PostgreSQL server version {}...", postgresql_version);

    // Setup PostgreSQL directories
    let installation_dir = config.installation_dir();
    let data_dir = config.data_dir();

    // Create installation directory if it doesn't exist
    if !installation_dir.exists() {
        println!("Creating installation directory: {}", installation_dir.display());
        fs::create_dir_all(&installation_dir)
            .map_err(|e| format!("Failed to create installation directory: {}", e))?;
    }

    // Create parent directory for data_dir if it doesn't exist
    if let Some(parent) = data_dir.parent() {
        if !parent.exists() {
            println!("Creating data directory parent: {}", parent.display());
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create data directory parent: {}", e))?;
        }
    }

    // Configure PostgreSQL settings
    use postgresql_embedded::{PostgreSQL, Settings, VersionReq};

    let mut settings = Settings::default();
    settings.version = VersionReq::parse(&format!("={}", postgresql_version))
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    settings.temporary = false;
    settings.installation_dir = installation_dir.clone();
    settings.data_dir = data_dir.clone();
    settings.username = config.postgresql.username.clone();
    settings.password = config.postgresql.password.clone();
    settings.port = config.postgresql.port;
    settings.host = config.postgresql.bind_address.clone();
    settings.configuration = std::collections::HashMap::new();

    // Enable comprehensive logging
    let logging_collector = if config.postgresql.logging.collector { "on" } else { "off" };
    settings
        .configuration
        .insert("logging_collector".to_string(), logging_collector.to_string());
    settings
        .configuration
        .insert("log_directory".to_string(), config.postgresql.logging.directory.clone());
    settings.configuration.insert(
        "log_filename".to_string(),
        config.postgresql.logging.filename.clone(),
    );
    settings
        .configuration
        .insert("log_statement".to_string(), config.postgresql.logging.statement.clone());

    // Remove existing data directory and recreate it for fresh data
    if settings.data_dir.exists() {
        // Check if postmaster.pid exists and kill the process if it does
        let postmaster_pid_path = settings.data_dir.join("postmaster.pid");
        if postmaster_pid_path.exists() {
            if let Ok(pid_content) = fs::read_to_string(&postmaster_pid_path) {
                // First line contains the PID
                if let Some(first_line) = pid_content.lines().next() {
                    if let Ok(pid) = first_line.trim().parse::<i32>() {
                        println!("Stopping existing PostgreSQL process (PID: {})...", pid);

                        // Try to kill the process cross-platform
                        #[cfg(target_os = "windows")]
                        let _ = Command::new("taskkill")
                            .arg("/F")
                            .arg("/PID")
                            .arg(pid.to_string())
                            .status();

                        #[cfg(not(target_os = "windows"))]
                        let _ = Command::new("kill")
                            .arg("-TERM")
                            .arg(pid.to_string())
                            .status();

                        // Give it a moment to shut down gracefully
                        thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }
        }

        println!("Removing existing data directory...");
        fs::remove_dir_all(&settings.data_dir)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    }
    fs::create_dir_all(&settings.data_dir)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // Set timezone from config
    settings
        .configuration
        .insert("timezone".to_string(), config.postgresql.timezone.clone());
    settings
        .configuration
        .insert("log_timezone".to_string(), config.postgresql.log_timezone.clone());

    // Create PostgreSQL instance
    let mut postgresql = PostgreSQL::new(settings);

    // Setup and start PostgreSQL
    println!("Setting up PostgreSQL...");
    match postgresql.setup().await {
        Ok(()) => println!("PostgreSQL setup completed"),
        Err(e) => {
            eprintln!("PostgreSQL setup failed: {}", e);
            return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
        }
    }

    println!("Starting PostgreSQL server...");
    match postgresql.start().await {
        Ok(()) => {
            println!("PostgreSQL server started successfully");
        }
        Err(e) => {
            eprintln!("Failed to start PostgreSQL server: {}", e);

            // Try to find and display log files
            let log_dir = data_dir.join("log");
            if log_dir.exists() {
                eprintln!("\nChecking log directory: {}", log_dir.display());
                if let Ok(entries) = fs::read_dir(&log_dir) {
                    for entry in entries.flatten() {
                        if let Ok(content) = fs::read_to_string(entry.path()) {
                            eprintln!("\n=== Log file: {} ===", entry.path().display());
                            eprintln!("{}", content);
                        }
                    }
                }
            }

            // Check if port is in use
            eprintln!("\nChecking if port {} is available...", config.postgresql.port);
            use std::net::TcpListener;
            match TcpListener::bind(format!("{}:{}", config.postgresql.bind_address, config.postgresql.port)) {
                Ok(_) => eprintln!("Port {} is available", config.postgresql.port),
                Err(e) => eprintln!("Port {} is NOT available: {}", config.postgresql.port, e),
            }

            return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
        }
    }

    let database_url = postgresql.settings().url("postgres");
    println!("Database URL: {}", database_url);

    // Connect to database and run migrations
    println!("Connecting to database...");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.postgresql.pool.max_connections)
        .min_connections(config.postgresql.pool.min_connections)
        .acquire_timeout(std::time::Duration::from_secs(config.postgresql.pool.acquire_timeout_secs))
        .connect(&database_url)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    println!("Database connected successfully");

    // Test connection
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // Run migrations from configured path
    let migrations_path = config.migrations_path();
    if !migrations_path.exists() {
        return Err(format!(
            "Migrations directory not found at: {}\nPlease ensure the migrations directory exists.",
            migrations_path.display()
        )
        .into());
    }

    println!("Running migrations from {}...", migrations_path.display());
    let migrator = sqlx::migrate::Migrator::new(migrations_path.as_path())
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    migrator
        .run(&pool)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    println!("Migrations applied successfully");

    println!("\n=================================================");
    println!("PostgreSQL Development Server Ready!");
    println!("=================================================");
    println!("Port: {}", config.postgresql.port);
    println!("Database URL: {}", database_url);
    println!("Username: {}", config.postgresql.username);
    println!("Password: {}", config.postgresql.password);
    println!("Database: {}", config.postgresql.database);
    println!("=================================================");
    println!("\nFor sqlx-cli commands, use:");
    println!("  export DATABASE_URL=\"{}\"", database_url);

    if keep_alive {
        println!("\nPress Ctrl+C to stop the server...\n");

        // Keep the server running indefinitely
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    } else {
        println!("\nMigrations completed. Server will now stop.\n");
        Ok(())
    }
}
