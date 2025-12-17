//! Engine trait and implementations

use async_trait::async_trait;
use std::process::Child;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::config::InstanceConfig;
use crate::error::Result;
use std::path::PathBuf;

pub mod llamacpp;
pub mod mistralrs;
pub mod mock;

/// Trait for inference engine implementations
#[async_trait]
pub trait Engine: Send + Sync {
    /// Get the name of this engine
    fn name(&self) -> &'static str;

    /// Get the version of this engine
    fn version(&self) -> String;

    /// Start an engine process with the given configuration
    /// Uses auto-discovery to find the binary
    async fn start(&self, config: &InstanceConfig) -> Result<EngineProcess>;

    /// Start an engine process with an explicit binary path
    /// This is used when the server manages binaries and passes explicit paths
    async fn start_with_binary(
        &self,
        config: &InstanceConfig,
        binary_path: PathBuf,
    ) -> Result<EngineProcess>;

    /// Stop an engine process
    async fn stop(&self, process: &mut EngineProcess) -> Result<()>;

    /// Perform a health check on a running engine
    async fn health_check(&self, process: &EngineProcess) -> Result<HealthStatus>;
}

/// A running engine process
#[derive(Debug)]
pub struct EngineProcess {
    /// Unique ID for this process instance
    pub id: Uuid,

    /// Instance configuration ID
    pub instance_id: String,

    /// Process ID
    pub pid: u32,

    /// Port the engine is listening on
    pub port: u16,

    /// Time when the process was started
    pub started_at: Instant,

    /// Child process handle (for cleanup)
    pub(crate) child: Child,

    /// Number of restart attempts
    pub restart_count: u32,
}

impl EngineProcess {
    /// Create a new engine process
    pub fn new(instance_id: String, port: u16, child: Child) -> Self {
        let pid = child.id();
        Self {
            id: Uuid::new_v4(),
            instance_id,
            pid,
            port,
            started_at: Instant::now(),
            child,
            restart_count: 0,
        }
    }

    /// Get the base URL for this engine
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Get the uptime of this process
    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Check if the process is still running
    pub fn is_running(&self) -> bool {
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            // Signal 0 is a null signal that can be used to check if process exists
            kill(Pid::from_raw(self.pid as i32), Signal::SIGURG).is_ok()
        }

        #[cfg(windows)]
        {
            // On Windows, try_wait will return Ok(None) if process is still running
            match self.child.try_wait() {
                Ok(None) => true,
                _ => false,
            }
        }
    }
}

/// Health status of an engine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Engine is healthy and responding
    Healthy,

    /// Engine is starting up
    Starting,

    /// Engine is unhealthy (but process still running)
    Unhealthy(String),

    /// Engine process has crashed
    Crashed,
}

impl HealthStatus {
    /// Check if status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if status indicates a problem
    pub fn is_problematic(&self) -> bool {
        matches!(self, Self::Unhealthy(_) | Self::Crashed)
    }
}

/// Handle to a running engine instance (returned to callers)
#[derive(Debug, Clone)]
pub struct EngineHandle {
    /// Instance ID
    pub instance_id: String,

    /// Port the engine is listening on
    pub port: u16,

    /// Base URL for API requests
    pub base_url: String,

    /// Process ID
    pub pid: u32,
}

impl EngineHandle {
    /// Create a handle from an engine process
    pub fn from_process(process: &EngineProcess) -> Self {
        Self {
            instance_id: process.instance_id.clone(),
            port: process.port,
            base_url: process.base_url(),
            pid: process.pid,
        }
    }
}

/// Information about a running instance
#[derive(Debug, Clone)]
pub struct InstanceInfo {
    /// Instance ID
    pub id: String,

    /// Process ID
    pub pid: u32,

    /// Port
    pub port: u16,

    /// Base URL
    pub base_url: String,

    /// Health status
    pub health: HealthStatus,

    /// Uptime in seconds
    pub uptime_secs: u64,

    /// Number of restarts
    pub restart_count: u32,
}

impl InstanceInfo {
    /// Create instance info from process and health status
    pub fn from_process(process: &EngineProcess, health: HealthStatus) -> Self {
        Self {
            id: process.instance_id.clone(),
            pid: process.pid,
            port: process.port,
            base_url: process.base_url(),
            health,
            uptime_secs: process.uptime().as_secs(),
            restart_count: process.restart_count,
        }
    }
}
