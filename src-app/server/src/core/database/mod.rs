use postgresql_embedded::{PostgreSQL, Settings, VersionReq};
use sqlx::PgPool;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::OnceCell;

const POSTGRES_VERSION: &str = "17.5.0";

static DATABASE_POOL: OnceCell<Arc<PgPool>> = OnceCell::const_new();
static POSTGRESQL_INSTANCE: OnceCell<Arc<Mutex<PostgreSQL>>> = OnceCell::const_new();
static CLEANUP_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Stop any running PostgreSQL instance by checking for postmaster.pid and using pg_ctl stop
fn stop_existing_postgres_instance(
    installation_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data_dir = installation_dir.join("data");
    let postmaster_pid_path = data_dir.join("postmaster.pid");

    if !postmaster_pid_path.exists() {
        println!("No postmaster.pid found, no existing PostgreSQL instance to stop");
        return Ok(());
    }

    println!("Found existing postmaster.pid, stopping PostgreSQL instance...");

    // Handle cross-platform executable naming
    let pg_ctl_exe = if cfg!(target_os = "windows") {
        "pg_ctl.exe"
    } else {
        "pg_ctl"
    };

    let pg_ctl_path = installation_dir
        .join(POSTGRES_VERSION)
        .join("bin")
        .join(pg_ctl_exe);

    // Check if pg_ctl executable exists
    if !pg_ctl_path.exists() {
        println!("Warning: pg_ctl executable not found at {:?}", pg_ctl_path);
        return Ok(());
    }

    let output = Command::new(&pg_ctl_path)
        .arg("stop")
        .arg("-D")
        .arg(&data_dir)
        .arg("-m")
        .arg("fast") // Use fast shutdown mode
        .output()?;

    if output.status.success() {
        println!("Successfully stopped existing PostgreSQL instance");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("Error: Failed to stop PostgreSQL instance. Exiting to prevent database corruption.");
        eprintln!("STDERR: {}", stderr);
        eprintln!("STDOUT: {}", stdout);
        std::process::exit(1);
    }

    // Wait a moment for the process to fully stop
    std::thread::sleep(std::time::Duration::from_millis(1000));

    Ok(())
}

pub async fn initialize_database(
    config: &crate::core::config::Config,
) -> Result<Arc<PgPool>, Box<dyn std::error::Error + Send + Sync>> {
    println!("Initializing database");

    let config_clone = config.clone();
    let pool = DATABASE_POOL
        .get_or_try_init(|| async move {
            // Retry logic for database initialization
            let max_retries = 5;
            let retry_delay = std::time::Duration::from_secs(3);

            for attempt in 1..=max_retries {
                println!(
                    "Database initialization attempt {} of {}",
                    attempt, max_retries
                );

                match try_initialize_database_once(&config_clone).await {
                    Ok(pool) => {
                        return Ok::<Arc<PgPool>, Box<dyn std::error::Error + Send + Sync>>(pool)
                    }
                    Err(e) => {
                        eprintln!("Database initialization attempt {} failed: {}", attempt, e);
                        if attempt < max_retries {
                            println!("Waiting {} seconds before retry...", retry_delay.as_secs());
                            tokio::time::sleep(retry_delay).await;
                        } else {
                            return Err(format!(
                                "Failed to initialize database after {} attempts: {}",
                                max_retries, e
                            )
                            .into());
                        }
                    }
                }
            }

            unreachable!()
        })
        .await?;

    //test query again to ensure the connection is valid after migrations
    let new_pool = get_database_pool()?;
    sqlx::query("SELECT 1").execute(new_pool.as_ref()).await?;

    println!("Database initialized successfully");

    Ok(pool.clone())
}

async fn try_initialize_database_once(
    config: &crate::core::config::Config,
) -> Result<Arc<PgPool>, Box<dyn std::error::Error + Send + Sync>> {
    let database_url = if config.postgresql.use_embedded {
        // Initialize embedded PostgreSQL
        let embedded = config.postgresql.embedded.as_ref()
            .ok_or("embedded config must be present when use_embedded is true")?;

        let mut settings = Settings::default();
        settings.version = VersionReq::parse(&format!("={}", embedded.version))?;
        settings.temporary = false;

        // Use directories from config
        settings.installation_dir = PathBuf::from(&embedded.installation_dir);

        // Stop any existing PostgreSQL instance before proceeding
        stop_existing_postgres_instance(&settings.installation_dir)?;

        settings.username = embedded.username.clone();
        settings.password_file = settings.installation_dir.join(".pgpass");
        if settings.password_file.exists() {
            settings.password = std::fs::read_to_string(settings.password_file.clone())?;
        } else {
            settings.password = embedded.password.clone();
        }
        settings.data_dir = PathBuf::from(&embedded.data_dir);

        // Set timezone from config
        settings
            .configuration
            .insert("timezone".to_string(), embedded.timezone.clone());
        settings
            .configuration
            .insert("log_timezone".to_string(), embedded.log_timezone.clone());

        // Use port and bind address from config
        settings.port = embedded.port;
        settings.host = embedded.bind_address.clone();

        // Set logging configuration from config
        let logging_collector = if embedded.logging.collector { "on" } else { "off" };
        settings
            .configuration
            .insert("logging_collector".to_string(), logging_collector.to_string());
        settings
            .configuration
            .insert("log_directory".to_string(), embedded.logging.directory.clone());
        settings
            .configuration
            .insert("log_filename".to_string(), embedded.logging.filename.clone());
        settings
            .configuration
            .insert("log_statement".to_string(), embedded.logging.statement.clone());

        let mut postgresql = PostgreSQL::new(settings);
        println!(
            "Setting up embedded PostgreSQL at port {}",
            postgresql.settings().port
        );

        postgresql.setup().await?;

        // Note: pgvector and Apache AGE extensions can be added later when needed

        println!("Starting embedded PostgreSQL...");
        postgresql.start().await?;

        let database_url = postgresql.settings().url("postgres");
        println!("Generated database_url: {:?}", database_url);

        // Store the PostgreSQL instance to keep it alive
        POSTGRESQL_INSTANCE
            .set(Arc::new(Mutex::new(postgresql)))
            .map_err(|_| "Failed to store PostgreSQL instance")?;

        // Register cleanup handlers once
        register_cleanup_handlers();

        // Initialize the static cleanup instance
        std::sync::LazyLock::force(&_CLEANUP);

        database_url
    } else {
        // Use external PostgreSQL
        let external = config.postgresql.external.as_ref()
            .ok_or("external config must be present when use_embedded is false")?;
        println!("Using external PostgreSQL at {}:{}",
            external.host,
            external.port);
        config.database_url()
    };

    // Wait for PostgreSQL to be ready with retry logic
    let pool = connect_with_retry(&database_url, config).await?;

    //test query to ensure the connection is valid
    println!("Testing database connection...");
    sqlx::query("SELECT 1").execute(&pool).await?;

    // Run migrations
    println!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(Arc::new(pool))
}

async fn connect_with_retry(
    database_url: &str,
    config: &crate::core::config::Config,
) -> Result<PgPool, Box<dyn std::error::Error + Send + Sync>> {
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;

    let max_retries = 10;
    let mut retry_count = 0;

    println!("Attempting to connect to database with retry logic...");

    // Configure connection pool with timeouts from config or defaults
    let pool_config = config.postgresql.pool.as_ref();
    let max_connections = pool_config.map(|p| p.max_connections).unwrap_or(10);
    let min_connections = pool_config.map(|p| p.min_connections).unwrap_or(1);
    let acquire_timeout_secs = pool_config.map(|p| p.acquire_timeout_secs).unwrap_or(5);

    let mut pool_options = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout_secs));

    if let Some(pool) = pool_config {
        if let Some(idle_timeout) = pool.idle_timeout_secs {
            pool_options = pool_options.idle_timeout(Duration::from_secs(idle_timeout));
        }

        if let Some(max_lifetime) = pool.max_lifetime_secs {
            pool_options = pool_options.max_lifetime(Duration::from_secs(max_lifetime));
        }
    }

    loop {
        retry_count += 1;
        println!("Connection attempt {} of {}", retry_count, max_retries);

        match pool_options.clone().connect(database_url).await {
            Ok(pool) => {
                println!(
                    "Successfully connected to database on attempt {}",
                    retry_count
                );

                // Test the connection with a simple query
                match sqlx::query("SELECT 1").execute(&pool).await {
                    Ok(_) => {
                        println!("Database connection test successful");
                        return Ok(pool);
                    }
                    Err(e) => {
                        println!("Database connection test failed: {}", e);
                        if retry_count >= max_retries {
                            return Err(format!(
                                "Database connection test failed after {} attempts: {}",
                                max_retries, e
                            )
                            .into());
                        }
                    }
                }
            }
            Err(e) => {
                println!("Connection attempt {} failed: {}", retry_count, e);
                if retry_count >= max_retries {
                    return Err(format!(
                        "Failed to connect to database after {} attempts: {}",
                        max_retries, e
                    )
                    .into());
                }
            }
        }

        // Wait before retrying (exponential backoff)
        let delay = Duration::from_millis(100 * (1 << (retry_count - 1).min(6))); // Cap at ~6.4 seconds
        println!("Waiting {:?} before retry...", delay);
        tokio::time::sleep(delay).await;
    }
}

pub fn get_database_pool() -> Result<Arc<PgPool>, sqlx::Error> {
    DATABASE_POOL
        .get()
        .cloned()
        .ok_or(sqlx::Error::PoolTimedOut)
}

pub async fn cleanup_database() {
    println!("Cleaning up database...");

    // Close the database pool
    if let Some(pool) = DATABASE_POOL.get() {
        pool.close().await;
        println!("Database pool closed");
    }

    // Stop the PostgreSQL instance
    if let Some(postgresql_arc) = POSTGRESQL_INSTANCE.get() {
        let postgresql_arc = postgresql_arc.clone();
        tokio::task::spawn_blocking(move || {
            if let Ok(postgresql) = postgresql_arc.lock() {
                let rt = tokio::runtime::Runtime::new().unwrap();
                if let Err(e) = rt.block_on(postgresql.stop()) {
                    eprintln!("Error stopping PostgreSQL: {}", e);
                } else {
                    println!("PostgreSQL instance stopped");
                }
            }
        })
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to stop PostgreSQL: {}", e);
        });
    }
}

fn register_cleanup_handlers() {
    // Only register once
    if CLEANUP_REGISTERED.swap(true, Ordering::SeqCst) {
        return;
    }

    // Register cleanup on panic
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        println!("Panic detected, cleaning up database...");
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cleanup_database());
        orig_hook(panic_info);
    }));
}

// Drop implementation for graceful shutdown
struct DatabaseCleanup;

impl Drop for DatabaseCleanup {
    fn drop(&mut self) {
        println!("DatabaseCleanup Drop called, cleaning up database...");
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cleanup_database());
    }
}

// Static instance to ensure cleanup on drop
static _CLEANUP: std::sync::LazyLock<DatabaseCleanup> =
    std::sync::LazyLock::new(|| DatabaseCleanup);
