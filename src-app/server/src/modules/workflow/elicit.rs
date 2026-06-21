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
    // L6: reject a submission that arrives past the deadline. The
    // dispatcher's timeout arm is authoritative, but a submission landing in
    // the skew window between deadline and the timer firing should be told
    // it's too late rather than delivered as a normal response.
    if chrono::Utc::now() > pending.deadline_at {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::GONE,
            "WORKFLOW_ELICIT_STALE",
            "elicitation deadline has passed",
        )).into());
    }

    // Validate the response against the persisted schema (plan §3:
    // "Validated against the schema → 422 on mismatch"). E5: full JSON-Schema
    // validation via `validate_response_shape` (enum/pattern/const/nested/…),
    // falling back to a structural check only for a malformed authoring schema.
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

/// Validate a value against a JSON Schema. E5: a full JSON-Schema engine (the
/// `jsonschema` crate — `enum` / `pattern` / `const` / nested objects+arrays /
/// `oneOf` / `minItems` / …). On a malformed AUTHORING schema (one that doesn't
/// compile as JSON Schema) it falls back to the lightweight structural check
/// below, so a bad schema never crashes or blocks the responder. Shared by the
/// elicit submit handler (422 on mismatch) and the `matches_schema:`
/// test-assertion mode (B6) so both stay in lockstep.
pub(crate) fn validate_response_shape(
    schema: &serde_json::Value,
    response: &serde_json::Value,
) -> Result<(), String> {
    // A non-object schema (`true`, absent, …) carries no constraints.
    if !schema.is_object() {
        return Ok(());
    }
    match jsonschema::validator_for(schema) {
        Ok(validator) => {
            // Collect up to 5 errors so the reviewer sees actionable detail
            // without an unbounded message.
            let msgs: Vec<String> = validator
                .iter_errors(response)
                .take(5)
                .map(|e| e.to_string())
                .collect();
            if msgs.is_empty() {
                Ok(())
            } else {
                Err(msgs.join("; "))
            }
        }
        Err(_) => validate_response_shape_lightweight(schema, response),
    }
}

/// Lightweight structural fallback: top-level `type` + `required` keys + each
/// declared property's primitive type + array `minItems`/`maxItems`. Used only
/// when the schema isn't valid JSON Schema (so `validator_for` fails).
fn validate_response_shape_lightweight(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn full_jsonschema_enforces_enum_required_and_type() {
        // E5: the jsonschema engine enforces enum/type/required — beyond the old
        // lightweight structural check.
        let schema = json!({
            "type": "object",
            "properties": {
                "decision": { "type": "string", "enum": ["include", "exclude"] },
                "score": { "type": "number" }
            },
            "required": ["decision"]
        });
        assert!(
            validate_response_shape(&schema, &json!({"decision": "include", "score": 0.9})).is_ok()
        );
        // enum violation
        assert!(validate_response_shape(&schema, &json!({"decision": "maybe"})).is_err());
        // type violation
        assert!(
            validate_response_shape(&schema, &json!({"decision": "include", "score": "hi"}))
                .is_err()
        );
        // missing required
        assert!(validate_response_shape(&schema, &json!({"score": 1})).is_err());
    }

    #[test]
    fn nested_array_of_objects_validated() {
        // The edited-table case: rows are array-of-object; full jsonschema
        // recurses into items (the old lightweight check did not).
        let schema = json!({
            "type": "object",
            "properties": {
                "rows": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "include": { "type": "boolean" } },
                        "required": ["include"]
                    }
                }
            }
        });
        assert!(validate_response_shape(&schema, &json!({"rows": [{"include": true}]})).is_ok());
        // nested per-row type violation
        assert!(validate_response_shape(&schema, &json!({"rows": [{"include": "yes"}]})).is_err());
    }

    #[test]
    fn non_object_schema_is_permissive() {
        // A `true` / non-object schema carries no constraints.
        assert!(validate_response_shape(&json!(true), &json!({"anything": 1})).is_ok());
    }
}
