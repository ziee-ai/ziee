// Deployment layer for managing local runtime instances

pub mod local;
pub mod manager;

pub use local::LocalDeployment;

use crate::common::AppError;

type AppResult<T> = Result<T, AppError>;
use sqlx::types::Uuid;

/// Trait for deployment strategies
#[async_trait::async_trait]
pub trait Deployment: Send + Sync {
    /// Start a model instance
    async fn start(
        &self,
        model_id: Uuid,
        engine_type: &str,
        model_path: &str,
        config: &serde_json::Value,
    ) -> AppResult<DeploymentResult>;

    /// Stop a model instance
    async fn stop(&self, model_id: Uuid) -> AppResult<()>;

    /// Get instance status
    async fn status(&self, model_id: Uuid) -> AppResult<InstanceStatus>;

    /// Health check
    async fn health_check(&self, base_url: &str) -> AppResult<bool>;

    /// Get logs
    async fn get_logs(&self, model_id: Uuid, lines: usize) -> AppResult<Vec<String>>;
}

#[derive(Debug, Clone)]
pub struct DeploymentResult {
    pub pid: i32,
    pub port: i32,
    pub base_url: String,
}

#[derive(Debug, Clone)]
pub struct InstanceStatus {
    pub running: bool,
    pub pid: Option<i32>,
    pub port: Option<i32>,
    pub uptime_seconds: Option<i64>,
}
