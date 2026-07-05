//! REST surface for the citation library (the UI + the lit_search handoff use
//! this; the model uses the MCP tools in `handlers.rs`). The batch
//! resolve/verify/dedup logic is shared with the MCP path via
//! `handlers::{add_one, verify_one}`.

use aide::transform::TransformOperation;
use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
};
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{SyncAction, SyncOrigin};

use super::format::{self, ExportFormat};
use super::models::{
    AttachCitationsRequest, BatchReport, ExportQuery, ExportResponse, ImportCitationsRequest,
    ListCitationsQuery, ListCitationsResponse, MutationResponse, StylesResponse,
    VerifyCitationsRequest, MAX_BATCH_ITEMS,
};
use super::permissions::{CitationsManage, CitationsUse};
use super::repository::CitationsRepository;
use super::{csl, handlers};

fn repo() -> CitationsRepository {
    CitationsRepository::new(Repos.pool().clone())
}

fn cap_check(n: usize) -> Result<(), AppError> {
    if n > MAX_BATCH_ITEMS {
        return Err(AppError::bad_request(
            "CITATIONS_BATCH_TOO_LARGE",
            format!("too many items ({n}); cap is {MAX_BATCH_ITEMS}"),
        ));
    }
    Ok(())
}

// ── list ────────────────────────────────────────────────────────────────────

pub async fn list_citations(
    auth: RequirePermissions<(CitationsUse,)>,
    Query(q): Query<ListCitationsQuery>,
) -> ApiResult<Json<ListCitationsResponse>> {
    let entries = repo().list_entries(auth.user.id, q.project_id).await?;
    Ok((StatusCode::OK, Json(ListCitationsResponse { entries })))
}

pub fn list_citations_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsUse,)>(op)
        .id("Citations.list")
        .tag("Citations")
        .summary("List bibliography entries (optionally a project's reference list)")
        .response::<200, Json<ListCitationsResponse>>()
}

// ── import / add ──────────────────────────────────────────────────────────────

pub async fn import_citations(
    auth: RequirePermissions<(CitationsManage,)>,
    origin: SyncOrigin,
    Json(body): Json<ImportCitationsRequest>,
) -> ApiResult<Json<BatchReport>> {
    cap_check(body.items.len())?;
    // Cross-tenant guard before any add_one attaches into the project.
    handlers::verify_project_owned(auth.user.id, body.project_id).await?;
    let repo = repo();
    let mut results = Vec::with_capacity(body.items.len());
    for it in &body.items {
        results.push(handlers::add_one(&repo, auth.user.id, body.project_id, it).await);
    }
    if results.iter().any(|r| r.entry_id.is_some()) {
        handlers::emit_library_changed(auth.user.id, SyncAction::Create, uuid::Uuid::nil(), origin.0);
    }
    Ok((StatusCode::OK, Json(BatchReport { results })))
}

pub fn import_citations_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsManage,)>(op)
        .id("Citations.import")
        .tag("Citations")
        .summary("Resolve + verify + dedup + add references (by identifier or CSL-JSON)")
        .response::<200, Json<BatchReport>>()
}

// ── verify (no persistence) ───────────────────────────────────────────────────

pub async fn verify_citations(
    _auth: RequirePermissions<(CitationsUse,)>,
    Json(body): Json<VerifyCitationsRequest>,
) -> ApiResult<Json<BatchReport>> {
    cap_check(body.items.len())?;
    let mut results = Vec::with_capacity(body.items.len());
    for it in &body.items {
        results.push(handlers::verify_one(it).await);
    }
    Ok((StatusCode::OK, Json(BatchReport { results })))
}

pub fn verify_citations_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsUse,)>(op)
        .id("Citations.verify")
        .tag("Citations")
        .summary("Verify references resolve to real records (the fabrication checker)")
        .response::<200, Json<BatchReport>>()
}

// ── reverify (re-resolve stored entries + PERSIST status) ─────────────────────

/// Re-resolve every stored entry (optionally a project's list) by its best
/// identifier and PERSIST the new verification_status. This is what the
/// library's "Verify all" button calls — unlike `/verify` (which is a
/// stateless check of an arbitrary list), this updates the stored badges.
pub async fn reverify_citations(
    auth: RequirePermissions<(CitationsManage,)>,
    origin: SyncOrigin,
    Query(q): Query<ListCitationsQuery>,
) -> ApiResult<Json<BatchReport>> {
    handlers::verify_project_owned(auth.user.id, q.project_id).await?;
    let repo = repo();
    let entries = repo.list_entries(auth.user.id, q.project_id).await?;
    let mut results = Vec::with_capacity(entries.len());
    let mut changed = false;
    for e in entries {
        let result = handlers::reverify_entry(&repo, auth.user.id, &e).await;
        if result.0 {
            changed = true;
        }
        results.push(result.1);
    }
    if changed {
        handlers::emit_library_changed(auth.user.id, SyncAction::Update, uuid::Uuid::nil(), origin.0);
    }
    Ok((StatusCode::OK, Json(BatchReport { results })))
}

pub fn reverify_citations_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsManage,)>(op)
        .id("Citations.reverify")
        .tag("Citations")
        .summary("Re-resolve stored entries and persist their verification status")
        .response::<200, Json<BatchReport>>()
}

// ── delete from library ───────────────────────────────────────────────────────

pub async fn delete_citation(
    auth: RequirePermissions<(CitationsManage,)>,
    origin: SyncOrigin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<MutationResponse>> {
    let deleted = repo().delete_entry(auth.user.id, id).await?;
    if deleted {
        handlers::emit_library_changed(auth.user.id, SyncAction::Delete, id, origin.0);
    }
    Ok((
        StatusCode::OK,
        Json(MutationResponse {
            ok: deleted,
            count: None,
        }),
    ))
}

pub fn delete_citation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsManage,)>(op)
        .id("Citations.delete")
        .tag("Citations")
        .summary("Delete an entry from the library (cascades its project links)")
        .response::<200, Json<MutationResponse>>()
}

// ── export ────────────────────────────────────────────────────────────────────

pub async fn export_citations(
    auth: RequirePermissions<(CitationsUse,)>,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Json<ExportResponse>> {
    let fmt = ExportFormat::parse(q.format.as_deref().unwrap_or("text"));
    // Only the `text` renderer consumes/cleans a CSL style temp file; don't
    // extract one for other formats (it would orphan a /tmp file).
    let style_path = if fmt == ExportFormat::Text {
        q.style.as_deref().and_then(csl::style_path)
    } else {
        None
    };
    let entries = repo().list_entries(auth.user.id, q.project_id).await?;
    let items = entries.into_iter().map(|e| e.csl_json).collect();
    let output = format::export(items, fmt, style_path).await?;
    Ok((
        StatusCode::OK,
        Json(ExportResponse {
            format: q.format.unwrap_or_else(|| "text".to_string()),
            output,
        }),
    ))
}

pub fn export_citations_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsUse,)>(op)
        .id("Citations.export")
        .tag("Citations")
        .summary("Export a reference list (csljson | bibtex | ris | text in a CSL style)")
        .response::<200, Json<ExportResponse>>()
}

// ── CSL styles ────────────────────────────────────────────────────────────────

pub async fn list_styles(
    _auth: RequirePermissions<(CitationsUse,)>,
) -> ApiResult<Json<StylesResponse>> {
    Ok((
        StatusCode::OK,
        Json(StylesResponse {
            styles: csl::list_styles(),
        }),
    ))
}

pub fn list_styles_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsUse,)>(op)
        .id("Citations.listStyles")
        .tag("Citations")
        .summary("List bundled CSL style names")
        .response::<200, Json<StylesResponse>>()
}

// ── project reference-list membership ─────────────────────────────────────────

pub async fn attach_to_project(
    auth: RequirePermissions<(CitationsManage,)>,
    origin: SyncOrigin,
    Path(project_id): Path<Uuid>,
    Json(body): Json<AttachCitationsRequest>,
) -> ApiResult<Json<MutationResponse>> {
    // Cross-tenant guard: the project must belong to the caller (404, not 403,
    // so project existence isn't leaked). Mirrors file/project_extension.
    handlers::verify_project_owned(auth.user.id, Some(project_id)).await?;
    let repo = repo();
    // Ownership filtering + inserts run in one transaction so a mid-batch
    // failure can't leave a partially-attached reference list.
    let count = repo
        .attach_many_to_project(auth.user.id, project_id, &body.entry_ids)
        .await?;
    if count > 0 {
        handlers::emit_library_changed(auth.user.id, SyncAction::Update, uuid::Uuid::nil(), origin.0);
    }
    Ok((
        StatusCode::OK,
        Json(MutationResponse {
            ok: true,
            count: Some(count),
        }),
    ))
}

pub fn attach_to_project_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsManage,)>(op)
        .id("Citations.attachToProject")
        .tag("Citations")
        .summary("Add existing library entries to a project's reference list")
        .response::<200, Json<MutationResponse>>()
}

pub async fn detach_from_project(
    auth: RequirePermissions<(CitationsManage,)>,
    origin: SyncOrigin,
    Path((project_id, entry_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<MutationResponse>> {
    handlers::verify_project_owned(auth.user.id, Some(project_id)).await?;
    repo().detach_from_project(project_id, entry_id).await?;
    handlers::emit_library_changed(auth.user.id, SyncAction::Update, entry_id, origin.0);
    Ok((
        StatusCode::OK,
        Json(MutationResponse {
            ok: true,
            count: None,
        }),
    ))
}

pub fn detach_from_project_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CitationsManage,)>(op)
        .id("Citations.detachFromProject")
        .tag("Citations")
        .summary("Unlink an entry from a project (the entry stays in the library)")
        .response::<200, Json<MutationResponse>>()
}
