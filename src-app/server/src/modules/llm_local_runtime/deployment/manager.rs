// Deployment manager - orchestrates local deployments

use super::{Deployment, LocalDeployment};
use crate::common::AppError;

type AppResult<T> = Result<T, AppError>;
use crate::modules::llm_local_runtime::models::DeploymentConfig;
use std::sync::Arc;

pub struct DeploymentManager {
    local: Arc<LocalDeployment>,
}

impl DeploymentManager {
    pub fn new() -> Self {
        Self {
            local: Arc::new(LocalDeployment::new()),
        }
    }

    /// Get deployment strategy based on configuration (currently only local deployment)
    pub async fn get_deployment(
        &self,
        _config: &DeploymentConfig,
    ) -> AppResult<Arc<dyn Deployment>> {
        Ok(self.local.clone())
    }
}

impl Default for DeploymentManager {
    fn default() -> Self {
        Self::new()
    }
}
