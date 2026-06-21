//! HTTP handlers: the JSON-RPC MCP endpoint for the citations server.
//!
//! `tools/call` dispatches the six batch tools — `lookup_citations` /
//! `add_citations` / `verify_citations` / `list_citations` / `format_citations`
//! / `remove_citations` — through the resolve/verify/dedup engine and the
//! repository. The batch orchestration (`add_one`/`verify_one`/`reverify_entry`)
//! is shared with the REST surface in `rest.rs`.

use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::core::Repos;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::permissions::RequirePermissions;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};

use super::models::{
    CitationInput, CitationItemResult, DedupOutcome, MAX_BATCH_ITEMS, VerificationStatus,
};
use super::permissions::CitationsUse;
use super::format::ExportFormat;
use super::repository::{CitationsRepository, NewEntry};
use super::{csl, dedup, format, resolve, verify};

#[debug_handler]
pub async fn jsonrpc_handler(
    // Gated on `citations::use`; the JWT is validated by the extractor. user_id
    // comes from the JWT (auth.user.id) — the library is per-user.
    auth: RequirePermissions<(CitationsUse,)>,
    ConversationIdHeader(_conversation_id): ConversationIdHeader,
    body: axum::body::Bytes,
) -> Response {
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::parse_error(e.to_string()),
            );
        }
    };
    let req: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::invalid_request(e.to_string()),
            );
        }
    };

    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let id = req.id.clone();
    let user_id = auth.user.id;

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "citations", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
        "tools/list" => ok_response(id, super::tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        "tools/call" => match dispatch_tool_call(user_id, &req.params).await {
            Ok(value) => ok_response(id, value),
            Err(e) => error_response(id, e.0, e.1),
        },
        _ => error_response(id, StatusCode::OK, JsonRpcError::method_not_found(&req.method)),
    }
}

fn ok_response(id: Option<Value>, result: Value) -> Response {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }),
    )
        .into_response()
}

fn error_response(id: Option<Value>, http: StatusCode, err: JsonRpcError) -> Response {
    (
        http,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    )
        .into_response()
}

/// Build the standard MCP tool-result envelope.
fn tool_result(text: String, structured: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
    })
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

fn arg_uuid(args: &Value, key: &str) -> Option<Uuid> {
    args.get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Notify the caller's other devices that their bibliography changed
/// (notify-and-refetch; owner-scoped). `id` is an affected entry id (or nil for
/// a batch). origin None — MCP tool / REST mutation, not a tab-echoed one.
pub(super) fn emit_library_changed(user_id: Uuid, action: SyncAction, id: Uuid) {
    sync_publish(
        SyncEntity::BibliographyEntry,
        action,
        id,
        Audience::owner(user_id),
        None,
    );
}

/// Confirm `project_id` belongs to `user_id` before any project reference-list
/// mutation. Load-bearing: without it a user could attach/detach entries on
/// another user's project (cross-tenant pollution). Mirrors
/// `file/project_extension/handlers.rs`. Returns `Ok(None)` when no project was
/// requested. 404 (not 403) on the cross-tenant case so project existence isn't
/// leaked.
pub(super) async fn verify_project_owned(
    user_id: Uuid,
    project_id: Option<Uuid>,
) -> Result<Option<Uuid>, crate::common::AppError> {
    match project_id {
        None => Ok(None),
        Some(pid) => {
            Repos
                .project
                .get_for_user(pid, user_id)
                .await?
                .ok_or_else(|| crate::common::AppError::not_found("Project"))?;
            Ok(Some(pid))
        }
    }
}

/// Dispatch one MCP tool call. The JSON-RPC endpoint gated the WHOLE surface on
/// `CitationsUse`; the mutating tools here (`add_citations`/`remove_citations`)
/// are therefore reachable with only `citations::use` — deliberately. The
/// library is strictly per-user (every query is `WHERE user_id = $1`) and the
/// model acts on the user's own data, so write-via-tool is the same trust level
/// as read. (REST mutations require `CitationsManage` because they're driven by
/// the human UI, where gating the affordance is the right UX.) Cross-tenant
/// project writes are independently blocked by `verify_project_owned`.
async fn dispatch_tool_call(
    user_id: Uuid,
    params: &Value,
) -> Result<Value, (StatusCode, JsonRpcError)> {
    let call: ToolCallParams = serde_json::from_value(params.clone())
        .map_err(|e| (StatusCode::OK, JsonRpcError::invalid_params(e.to_string())))?;
    let repo = CitationsRepository::new(Repos.pool().clone());

    match call.name.as_str() {
        "list_citations" => {
            let project_id = arg_uuid(&call.arguments, "project_id");
            let entries = repo
                .list_entries(user_id, project_id)
                .await
                .map_err(internal)?;
            let text = format!("{} citation(s) in the bibliography.", entries.len());
            Ok(tool_result(text, json!({ "entries": entries })))
        }
        "remove_citations" => {
            let project_id = arg_uuid(&call.arguments, "project_id");
            verify_project_owned(user_id, project_id).await.map_err(internal)?;
            let ids: Vec<Uuid> = call
                .arguments
                .get("ids")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .filter_map(|s| Uuid::parse_str(s).ok())
                        .collect()
                })
                .unwrap_or_default();
            let mut removed = 0usize;
            for id in &ids {
                match project_id {
                    Some(pid) => {
                        repo.detach_from_project(pid, *id).await.map_err(internal)?;
                        removed += 1;
                    }
                    None => {
                        if repo.delete_entry(user_id, *id).await.map_err(internal)? {
                            removed += 1;
                        }
                    }
                }
            }
            let verb = if project_id.is_some() { "unlinked" } else { "deleted" };
            if removed > 0 {
                emit_library_changed(user_id, SyncAction::Delete, Uuid::nil());
            }
            Ok(tool_result(
                format!("{removed} citation(s) {verb}."),
                json!({ "removed": removed }),
            ))
        }
        "lookup_citations" => {
            let items = parse_items(&call.arguments)?;
            let mut out = Vec::with_capacity(items.len());
            for it in &items {
                out.push(lookup_one(it).await);
            }
            Ok(tool_result(summarize(&out), json!({ "results": out })))
        }
        "add_citations" => {
            let project_id = arg_uuid(&call.arguments, "project_id");
            verify_project_owned(user_id, project_id).await.map_err(internal)?;
            let items = parse_items(&call.arguments)?;
            let mut out = Vec::with_capacity(items.len());
            for it in &items {
                out.push(add_one(&repo, user_id, project_id, it).await);
            }
            if out.iter().any(|r| r.entry_id.is_some()) {
                emit_library_changed(user_id, SyncAction::Create, Uuid::nil());
            }
            Ok(tool_result(summarize(&out), json!({ "results": out })))
        }
        "verify_citations" => {
            // The fabrication checker: resolve each item, report status, persist nothing.
            let items = parse_items(&call.arguments)?;
            let mut out = Vec::with_capacity(items.len());
            for it in &items {
                out.push(verify_one(it).await);
            }
            Ok(tool_result(summarize(&out), json!({ "results": out })))
        }
        "format_citations" => {
            let project_id = arg_uuid(&call.arguments, "project_id");
            let fmt = ExportFormat::parse(
                call.arguments
                    .get("format")
                    .and_then(|v| v.as_str())
                    .unwrap_or("text"),
            );
            // Only the `text` renderer consumes (and cleans up) a CSL style file;
            // extracting it for other formats would orphan a temp file.
            let style_path = if fmt == ExportFormat::Text {
                call.arguments
                    .get("style")
                    .and_then(|v| v.as_str())
                    .and_then(csl::style_path)
            } else {
                None
            };
            let ids: Vec<Uuid> = call
                .arguments
                .get("ids")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .filter_map(|s| Uuid::parse_str(s).ok())
                        .collect()
                })
                .unwrap_or_default();
            let entries = if ids.is_empty() {
                repo.list_entries(user_id, project_id).await.map_err(internal)?
            } else {
                let mut v = Vec::new();
                for id in ids {
                    if let Some(e) = repo.get_entry(user_id, id).await.map_err(internal)? {
                        v.push(e);
                    }
                }
                v
            };
            let items: Vec<Value> = entries.iter().map(|e| e.csl_json.clone()).collect();
            let n = items.len();
            let output = format::export(items, fmt, style_path).await.map_err(internal)?;
            Ok(tool_result(
                format!("Formatted {n} reference(s)."),
                json!({ "output": output }),
            ))
        }
        other => Err((StatusCode::OK, JsonRpcError::method_not_found(other))),
    }
}

// ─────────────────────────── batch orchestration ───────────────────────────

fn parse_items(args: &Value) -> Result<Vec<CitationInput>, (StatusCode, JsonRpcError)> {
    let arr = args
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            (
                StatusCode::OK,
                JsonRpcError::invalid_params("missing `items` array".to_string()),
            )
        })?;
    if arr.len() > MAX_BATCH_ITEMS {
        return Err((
            StatusCode::OK,
            JsonRpcError::invalid_params(format!(
                "too many items ({}); cap is {MAX_BATCH_ITEMS}. Split into batches.",
                arr.len()
            )),
        ));
    }
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        let item: CitationInput = serde_json::from_value(v.clone())
            .map_err(|e| (StatusCode::OK, JsonRpcError::invalid_params(e.to_string())))?;
        out.push(item);
    }
    Ok(out)
}

fn item_label(input: &CitationInput) -> String {
    input
        .id
        .clone()
        .or_else(|| input.title.clone())
        .or_else(|| input.raw.clone())
        .or_else(|| input.csl.as_ref().and_then(resolve::csl_title))
        .unwrap_or_else(|| "(citation)".to_string())
}

fn summarize(results: &[CitationItemResult]) -> String {
    let mut verified = 0;
    let mut not_found = 0;
    let mut mismatch = 0;
    let mut other = 0;
    for r in results {
        match r.verification_status {
            VerificationStatus::Verified => verified += 1,
            VerificationStatus::NotFound => not_found += 1,
            VerificationStatus::Mismatch => mismatch += 1,
            VerificationStatus::Unverified => other += 1,
        }
    }
    format!(
        "{} item(s): {verified} verified, {mismatch} mismatch, {not_found} not found, {other} unverified.",
        results.len()
    )
}

fn linked(
    label: String,
    eid: Uuid,
    status: VerificationStatus,
    mismatch: &[String],
) -> CitationItemResult {
    CitationItemResult {
        input: label,
        entry_id: Some(eid),
        citation_key: None,
        dedup_outcome: Some(DedupOutcome::LinkedExisting),
        verification_status: status,
        possible_duplicate_of: None,
        mismatch_fields: (!mismatch.is_empty()).then(|| mismatch.to_vec()),
        reason: None,
    }
}

fn failed(label: String, reason: String) -> CitationItemResult {
    CitationItemResult {
        input: label,
        entry_id: None,
        citation_key: None,
        dedup_outcome: Some(DedupOutcome::Failed),
        verification_status: VerificationStatus::Unverified,
        possible_duplicate_of: None,
        mismatch_fields: None,
        reason: Some(reason),
    }
}

pub(super) async fn lookup_one(input: &CitationInput) -> CitationItemResult {
    let label = item_label(input);
    match resolve::resolve_input(input).await {
        Ok(r) => CitationItemResult {
            input: label,
            entry_id: None,
            citation_key: None,
            dedup_outcome: None,
            verification_status: r.status,
            possible_duplicate_of: None,
            mismatch_fields: (!r.mismatch_fields.is_empty()).then_some(r.mismatch_fields),
            reason: None,
        },
        Err(e) => failed(label, format!("{e}")),
    }
}

pub(super) async fn verify_one(input: &CitationInput) -> CitationItemResult {
    // Verify is lookup without persistence — same per-item status.
    lookup_one(input).await
}

/// Re-resolve a STORED entry by its best identifier and persist the new
/// verification status. Returns `(changed, report)`. Used by the library's
/// "Verify all" (the persisting counterpart to the stateless `verify_one`).
pub(super) async fn reverify_entry(
    repo: &CitationsRepository,
    user_id: Uuid,
    entry: &super::models::BibliographyEntry,
) -> (bool, CitationItemResult) {
    let input = if let Some(doi) = &entry.doi {
        CitationInput { id: Some(doi.clone()), ..Default::default() }
    } else if let Some(pmid) = &entry.pmid {
        CitationInput { id: Some(pmid.clone()), kind: Some("pmid".into()), ..Default::default() }
    } else if let Some(title) = &entry.title {
        CitationInput { title: Some(title.clone()), ..Default::default() }
    } else {
        // Nothing to resolve against — leave it as-is.
        return (
            false,
            CitationItemResult {
                input: entry.citation_key.clone(),
                entry_id: Some(entry.id),
                citation_key: Some(entry.citation_key.clone()),
                dedup_outcome: None,
                verification_status: entry.verification_status,
                possible_duplicate_of: None,
                mismatch_fields: None,
                reason: None,
            },
        );
    };

    let resolved = match resolve::resolve_input(&input).await {
        Ok(r) => r,
        Err(_) => {
            return (
                false,
                CitationItemResult {
                    input: entry.citation_key.clone(),
                    entry_id: Some(entry.id),
                    citation_key: Some(entry.citation_key.clone()),
                    dedup_outcome: None,
                    verification_status: entry.verification_status,
                    possible_duplicate_of: None,
                    mismatch_fields: None,
                    reason: Some("re-resolution failed".into()),
                },
            );
        }
    };

    // Guard against a transient demotion: for an identifier-less entry, a
    // free-text title search that misses returns `unverified` even for a record
    // that was previously legitimately verified — a flaky search must NOT flip a
    // `verified` entry back to `unverified`. So skip a downgrade-to-unverified
    // when the entry has no identifier to re-confirm against. (DOI/PMID entries
    // re-resolve deterministically, so their transitions are trusted.)
    let id_less = entry.doi.is_none() && entry.pmid.is_none();
    let is_transient_downgrade = id_less
        && entry.verification_status == VerificationStatus::Verified
        && resolved.status == VerificationStatus::Unverified;
    let final_status = if is_transient_downgrade {
        entry.verification_status
    } else {
        resolved.status
    };

    let changed = final_status != entry.verification_status;
    if changed {
        let _ = repo.set_verification(user_id, entry.id, final_status).await;
    }
    (
        changed,
        CitationItemResult {
            input: entry.citation_key.clone(),
            entry_id: Some(entry.id),
            citation_key: Some(entry.citation_key.clone()),
            dedup_outcome: None,
            verification_status: final_status,
            possible_duplicate_of: None,
            mismatch_fields: (!resolved.mismatch_fields.is_empty())
                .then(|| resolved.mismatch_fields.clone()),
            reason: None,
        },
    )
}

pub(super) async fn add_one(
    repo: &CitationsRepository,
    user_id: Uuid,
    project_id: Option<Uuid>,
    input: &CitationInput,
) -> CitationItemResult {
    let label = item_label(input);
    let resolved = match resolve::resolve_input(input).await {
        Ok(r) => r,
        Err(e) => return failed(label, format!("{e}")),
    };

    // A supplied identifier that doesn't resolve is a fabrication — never add it.
    if resolved.status == VerificationStatus::NotFound {
        return CitationItemResult {
            input: label,
            entry_id: None,
            citation_key: None,
            dedup_outcome: Some(DedupOutcome::Failed),
            verification_status: VerificationStatus::NotFound,
            possible_duplicate_of: None,
            mismatch_fields: None,
            reason: Some("identifier did not resolve to a real record".to_string()),
        };
    }

    let csl = resolved
        .csl
        .clone()
        .unwrap_or_else(|| json!({ "type": "article-journal" }));
    let title = resolve::csl_title(&csl);
    let year = resolve::csl_year(&csl);
    let surname = dedup::first_author_surname(&csl);

    // Dedup: DOI → PMID → exact fingerprint (link existing); else fuzzy review.
    if let Some(doi) = &resolved.doi {
        if let Ok(Some(eid)) = repo.find_by_doi(user_id, doi).await {
            if let Some(pid) = project_id {
                let _ = repo.attach_to_project(pid, eid).await;
            }
            return linked(label.clone(), eid, resolved.status, &resolved.mismatch_fields);
        }
    } else if let Some(pmid) = &resolved.pmid {
        if let Ok(Some(eid)) = repo.find_by_pmid(user_id, pmid).await {
            if let Some(pid) = project_id {
                let _ = repo.attach_to_project(pid, eid).await;
            }
            return linked(label.clone(), eid, resolved.status, &resolved.mismatch_fields);
        }
    }

    let fingerprint = if resolved.doi.is_none() && resolved.pmid.is_none() {
        Some(dedup::fingerprint(
            title.as_deref().unwrap_or(""),
            surname.as_deref(),
            year,
        ))
    } else {
        None
    };
    if let Some(fp) = &fingerprint {
        if let Ok(Some(eid)) = repo.find_by_fingerprint(user_id, fp).await {
            if let Some(pid) = project_id {
                let _ = repo.attach_to_project(pid, eid).await;
            }
            return linked(label.clone(), eid, resolved.status, &resolved.mismatch_fields);
        }
        // Fuzzy near-match → surface for review, do NOT auto-merge.
        if let Ok(cands) = repo.idless_candidates(user_id, year).await {
            for (cid, ctitle) in cands {
                if let (Some(t), Some(ct)) = (title.as_deref(), ctitle.as_deref()) {
                    if verify::title_matches(t, ct) {
                        return CitationItemResult {
                            input: label,
                            entry_id: None,
                            citation_key: None,
                            dedup_outcome: Some(DedupOutcome::PossibleDuplicate),
                            verification_status: resolved.status,
                            possible_duplicate_of: Some(cid),
                            mismatch_fields: None,
                            reason: Some("near-duplicate of an existing entry; review".to_string()),
                        };
                    }
                }
            }
        }
    }

    // Insert, tolerant of two distinct concurrency races (both surface as a 409
    // from the partial-unique indexes — see migration 102):
    //  * same DOI/PMID/fingerprint  → a real duplicate → re-find + link to it.
    //  * same citation_key, different work (two same-author/year papers added
    //    concurrently both pick `smith2021a`) → NOT a duplicate → regenerate the
    //    key and retry. A bounded loop keeps it from spinning.
    for attempt in 0..4 {
        let prefix = dedup::citation_key_base(surname.as_deref(), year);
        let existing_keys = repo
            .existing_citation_keys(user_id, &prefix)
            .await
            .unwrap_or_default();
        let citation_key = dedup::gen_citation_key(surname.as_deref(), year, &existing_keys);

        let new = NewEntry {
            csl_json: csl.clone(),
            doi: resolved.doi.clone(),
            pmid: resolved.pmid.clone(),
            pmcid: resolved.pmcid.clone(),
            arxiv_id: resolved.arxiv_id.clone(),
            title: title.clone(),
            year,
            dedup_fingerprint: fingerprint.clone(),
            citation_key,
            verification_status: resolved.status,
            source: Some(item_source(input).to_string()),
        };
        match repo.insert_entry(user_id, &new).await {
            Ok(entry) => {
                if let Some(pid) = project_id {
                    let _ = repo.attach_to_project(pid, entry.id).await;
                }
                return CitationItemResult {
                    input: label,
                    entry_id: Some(entry.id),
                    citation_key: Some(entry.citation_key),
                    dedup_outcome: Some(DedupOutcome::Inserted),
                    verification_status: resolved.status,
                    possible_duplicate_of: None,
                    mismatch_fields: (!resolved.mismatch_fields.is_empty())
                        .then(|| resolved.mismatch_fields.clone()),
                    reason: None,
                };
            }
            Err(e) if e.status_code() == 409 => {
                // Is it a real duplicate (DOI/PMID/fingerprint already present)?
                let existing = if let Some(doi) = &resolved.doi {
                    repo.find_by_doi(user_id, doi).await.ok().flatten()
                } else if let Some(pmid) = &resolved.pmid {
                    repo.find_by_pmid(user_id, pmid).await.ok().flatten()
                } else if let Some(fp) = &fingerprint {
                    repo.find_by_fingerprint(user_id, fp).await.ok().flatten()
                } else {
                    None
                };
                if let Some(eid) = existing {
                    if let Some(pid) = project_id {
                        let _ = repo.attach_to_project(pid, eid).await;
                    }
                    return linked(label, eid, resolved.status, &resolved.mismatch_fields);
                }
                // Not a duplicate → must be the citation_key index racing. Loop to
                // regenerate the key (re-querying existing keys) and retry.
                if attempt == 3 {
                    return failed(label, "could not store citation (key contention)".to_string());
                }
                continue;
            }
            Err(_) => return failed(label, "could not store citation".to_string()),
        }
    }
    failed(label, "could not store citation".to_string())
}

fn item_source(input: &CitationInput) -> &'static str {
    if input.csl.is_some() {
        "manual"
    } else if input.id.is_some() {
        "doi"
    } else {
        "manual"
    }
}

/// Map an AppError to a JSON-RPC error using the shared mapper (4xx →
/// invalid_params, else internal; safe `Display`, no Debug leak) — matches
/// web_search/lit_search. Carried over the wire at HTTP 200 (JSON-RPC in-band).
fn internal(e: crate::common::AppError) -> (StatusCode, JsonRpcError) {
    (StatusCode::OK, JsonRpcError::from_app_error(&e))
}
