//! Workflow REST DTOs. Phase B6 fills out the full surface
//! (dry-run, test, run, validate, etc.); B2 stubs the install
//! request types needed by the hub install handlers.

#![allow(dead_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::Workflow;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateWorkflowFromHubRequest {
    pub hub_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateSystemWorkflowFromHubRequest {
    pub hub_id: String,
    #[serde(default)]
    pub groups: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkflowFromHubResponse {
    pub workflow: Workflow,
    pub hub_tracking: crate::modules::hub::models::HubEntity,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkflowListResponse {
    pub workflows: Vec<Workflow>,
}
