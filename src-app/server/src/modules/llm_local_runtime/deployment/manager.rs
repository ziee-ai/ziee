// Deployment manager - orchestrates local deployments

use super::{Deployment, LocalDeployment};
use crate::common::AppError;
use sqlx::PgPool;
use crate::modules::llm_local_runtime::BinaryManager;

type AppResult<T> = Result<T, AppError>;
use crate::modules::llm_local_runtime::models::DeploymentConfig;
use std::sync::Arc;

pub struct DeploymentManager {
    local: Arc<LocalDeployment>,
}

impl DeploymentManager {
    pub fn new(pool: PgPool) -> Result<Self, Box<dyn std::error::Error>> {
        let binary_manager = BinaryManager::new(pool)?;
        Ok(Self {
            local: Arc::new(LocalDeployment::new(Arc::new(binary_manager))),
        })
    }

    /// Get deployment strategy based on configuration (currently only local deployment)
    pub async fn get_deployment(
        &self,
        _config: &DeploymentConfig,
    ) -> AppResult<Arc<dyn Deployment>> {
        Ok(self.local.clone())
    }
}
