// Branch API request/response types

use serde::Deserialize;
use uuid::Uuid;

/// Request to create a new branch (for edit/regenerate)
/// Both parent_branch_id (from conversation's active branch) and from_message_id are required
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateBranchRequest {
    pub from_message_id: Uuid,
}
