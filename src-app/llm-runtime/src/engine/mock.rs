//! Mock engine for testing (placeholder for Phase 2)

use async_trait::async_trait;

use super::{Engine, EngineProcess, HealthStatus};
use crate::config::InstanceConfig;
use crate::error::Result;

/// Mock engine for testing
pub struct MockEngine;

impl MockEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Engine for MockEngine {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn version(&self) -> String {
        "0.1.0-test".to_string()
    }

    async fn start(&self, _config: &InstanceConfig) -> Result<EngineProcess> {
        todo!("Implement in Phase 2")
    }

    async fn start_with_binary(
        &self,
        _config: &InstanceConfig,
        _binary_path: std::path::PathBuf,
    ) -> Result<EngineProcess> {
        todo!("Implement in Phase 2")
    }

    async fn stop(&self, _process: &mut EngineProcess) -> Result<()> {
        todo!("Implement in Phase 3")
    }

    async fn health_check(&self, _process: &EngineProcess) -> Result<HealthStatus> {
        todo!("Implement in Phase 3")
    }
}
