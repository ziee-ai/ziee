//! `POST /api/workflow-runs/{run_id}/elicit/{elicitation_id}` —
//! deliver user-submitted form response into the runner's
//! `ElicitDispatcher` (plan §4.6).
//!
//! Auth: caller must own the run (same gate as `/cancel`).
//! Staleness: the `elicitation_id` MUST match the pending one — old /
//! replayed responses return 410 Gone.
//! Validation: response is shape-checked against the persisted JSON
//! Schema before forwarding to the dispatcher.

#![allow(dead_code)]

use aide::transform::TransformOperation;
use axum::extract::Path as AxumPath;
use axum::http::StatusCode;
use axum::Json;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::workflow::permissions::WorkflowsExecute;
use crate::modules::workflow::registry;
use crate::modules::workflow::repository;
use crate::modules::workflow::types::{
    ElicitationResponseRequest, PendingElicitationRecord,
};

#[derive(Debug, Serialize, JsonSchema)]
pub struct ElicitAckResponse {
    pub status: String,
    pub run_id: Uuid,
    pub elicitation_id: Uuid,
}

pub async fn submit_elicit(
    auth: RequirePermissions<(WorkflowsExecute,)>,
    AxumPath((run_id, elicitation_id)): AxumPath<(Uuid, Uuid)>,
    Json(req): Json<ElicitationResponseRequest>,
) -> ApiResult<Json<ElicitAckResponse>> {
    let row = repository::find_run(Repos.pool(), run_id)
        .await?
        .ok_or_else(|| AppError::not_found("WorkflowRun"))?;
    if row.user_id != auth.user.id {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_RUN_FORBIDDEN",
            "workflow run is owned by another user",
        )).into());
    }

    // Pending record check.
    let pending: PendingElicitationRecord = match row.pending_elicitation_json.clone() {
        Some(v) => serde_json::from_value(v)
            .map_err(|e| AppError::internal_error(format!("decode pending elicit: {e}")))?,
        None => {
            return Err::<_, (StatusCode, AppError)>((AppError::new(
                StatusCode::GONE,
                "WORKFLOW_ELICIT_STALE",
                "no pending elicitation for this run",
            )).into());
        }
    };
    if pending.elicitation_id != elicitation_id {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::GONE,
            "WORKFLOW_ELICIT_STALE",
            "elicitation_id no longer pending (already resolved or replaced)",
        )).into());
    }

    // Validate the response against the persisted schema (plan §3:
    // "Validated against the schema → 422 on mismatch"). Lightweight
    // structural check — type + required-keys — covers the simple object
    // schemas elicit uses ({proceed: boolean, ...}). A full JSON Schema
    // engine (jsonschema-rs) is a future upgrade; not pulled in as a
    // dependency for this single call site.
    if let Err(msg) = validate_response_shape(&pending.schema, &req.response) {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "WORKFLOW_ELICIT_SCHEMA_MISMATCH",
            msg,
        )).into());
    }

    // Forward to the runner.
    match registry::submit_elicitation_response(run_id, elicitation_id, req.response) {
        Ok(()) => Ok((
            StatusCode::OK,
            Json(ElicitAckResponse {
                status: "delivered".into(),
                run_id,
                elicitation_id,
            }),
        )),
        Err("stale") => Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::GONE,
            "WORKFLOW_ELICIT_STALE",
            "elicitation_id no longer pending",
        )).into()),
        Err("none") => Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::GONE,
            "WORKFLOW_ELICIT_STALE",
            "no pending elicitation",
        )).into()),
        Err(other) => Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "WORKFLOW_ELICIT_DELIVER_FAILED",
            format!("submit elicit: {other}"),
        )).into()),
    }
}

/// Lightweight structural validation of a value against a JSON Schema.
/// Checks the top-level `type` + `required` keys + each declared
/// property's primitive type + array `minItems` / `maxItems`. Not a full
/// JSON Schema engine — sufficient for the simple object schemas elicit
/// steps use AND for the `matches_schema:` assertion mode in workflow
/// `tests/*.yaml` fixtures (B6 — see plan §7). Shared rather than
/// duplicated so the two call sites stay in lockstep.
pub(crate) fn validate_response_shape(
    schema: &serde_json::Value,
    response: &serde_json::Value,
) -> Result<(), String> {
    use serde_json::Value;
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return Ok(()), // no schema constraints
    };

    // Top-level type check.
    if let Some(Value::String(ty)) = obj.get("type") {
        let ok = match ty.as_str() {
            "object" => response.is_object(),
            "array" => response.is_array(),
            "string" => response.is_string(),
            "number" => response.is_number(),
            "integer" => response.is_i64() || response.is_u64(),
            "boolean" => response.is_boolean(),
            "null" => response.is_null(),
            _ => true,
        };
        if !ok {
            return Err(format!("response is not of type '{ty}'"));
        }
    }

    // Array item-count bounds.
    if let Some(arr) = response.as_array() {
        if let Some(min) = obj.get("minItems").and_then(|v| v.as_u64()) {
            if (arr.len() as u64) < min {
                return Err(format!(
                    "array has {} items, fewer than minItems {min}",
                    arr.len()
                ));
            }
        }
        if let Some(max) = obj.get("maxItems").and_then(|v| v.as_u64()) {
            if (arr.len() as u64) > max {
                return Err(format!(
                    "array has {} items, more than maxItems {max}",
                    arr.len()
                ));
            }
        }
    }

    // Required keys present (object schemas only).
    if let (Some(Value::Array(required)), Some(resp_obj)) =
        (obj.get("required"), response.as_object())
    {
        for r in required {
            if let Value::String(key) = r {
                if !resp_obj.contains_key(key) {
                    return Err(format!("missing required field '{key}'"));
                }
            }
        }
    }

    // Per-property primitive type check (best-effort).
    if let (Some(Value::Object(props)), Some(resp_obj)) =
        (obj.get("properties"), response.as_object())
    {
        for (key, prop_schema) in props {
            if let (Some(val), Some(Value::String(ty))) =
                (resp_obj.get(key), prop_schema.get("type"))
            {
                let ok = match ty.as_str() {
                    "object" => val.is_object(),
                    "array" => val.is_array(),
                    "string" => val.is_string(),
                    "number" => val.is_number(),
                    "integer" => val.is_i64() || val.is_u64(),
                    "boolean" => val.is_boolean(),
                    "null" => val.is_null(),
                    _ => true,
                };
                if !ok {
                    return Err(format!("field '{key}' is not of type '{ty}'"));
                }
            }
        }
    }

    Ok(())
}

pub fn submit_elicit_docs(op: TransformOperation) -> TransformOperation {
    crate::modules::permissions::with_permission::<(WorkflowsExecute,)>(op)
        .id("Workflow.submitElicit")
        .tag("Workflows - Runs")
        .summary("Submit a user response to a pending elicit step")
        .response::<200, Json<ElicitAckResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden"))
        .response_with::<404, (), _>(|r| r.description("Run not found"))
        .response_with::<410, (), _>(|r| r.description("Elicitation already resolved"))
}
