use aide::transform::TransformOperation;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::types::PermissionList;

/// 403 Forbidden response for missing permissions
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PermissionError {
    pub error: String,
    pub error_code: String,
    pub details: PermissionErrorDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PermissionErrorDetails {
    pub required_permissions: Vec<PermissionDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PermissionDetail {
    pub name: String,
    pub value: String,
    pub description: String,
}

/// Helper function to add permission info to OpenAPI operations
///
/// This enhances the OpenAPI spec with:
/// - Enhanced descriptions mentioning the required permission
/// - Proper 403 Forbidden response documentation
/// - Security requirement for Bearer token
///
/// # Example
/// ```rust
/// use crate::modules::permissions::openapi::with_permission;
/// use crate::modules::user::permissions::UsersRead;
///
/// fn list_users_docs(op: TransformOperation) -> TransformOperation {
///     with_permission::<UsersRead>(op)
///         .tag("Admin - Users")
///         .summary("List all users with pagination")
///         .response::<200, Json<UserListResponse>>()
/// }
/// ```
pub fn with_permission<Perms: PermissionList>(
    op: TransformOperation
) -> TransformOperation {
    // Add description with permission info
    let permission_desc = Perms::format_description();

    let op = op.description(&permission_desc);

    // Add standard 403 Forbidden response with JSON body
    let names = Perms::names();
    let permissions = Perms::permissions();
    let descriptions = Perms::descriptions();

    // Create example with permissions
    let error_msg = if permissions.len() == 1 {
        format!("Missing required permission: {}", permissions[0])
    } else {
        format!("Missing required permissions: {}", permissions.join(", "))
    };

    let permission_details: Vec<PermissionDetail> = names
        .iter()
        .zip(permissions.iter())
        .zip(descriptions.iter())
        .map(|((name, perm), desc)| PermissionDetail {
            name: name.to_string(),
            value: perm.to_string(),
            description: desc.to_string(),
        })
        .collect();

    let example_details = PermissionErrorDetails {
        required_permissions: permission_details,
    };

    let op = op.response_with::<403, Json<PermissionError>, _>(move |res| {
        res.description("Forbidden - Missing required permission")
            .example(PermissionError {
                error: error_msg.clone(),
                error_code: "INSUFFICIENT_PERMISSIONS".to_string(),
                details: example_details.clone(),
            })
    });

    // Add security requirement for Bearer token
    op.security_requirement("bearerAuth")
}

