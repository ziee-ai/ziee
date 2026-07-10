//! Deployment layer for the single managed whisper-server instance.

pub mod local;
pub mod manager;

pub use local::LocalDeployment;
pub use manager::get_deployment_manager;
