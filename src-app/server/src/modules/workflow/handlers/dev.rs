//! Dev / test REST handlers (Phase B6):
//!   POST /api/workflows/validate          (no side effect)
//!   POST /api/workflows/import            (multipart dev install)
//!   POST /api/workflows/{id}/dry-run      (cost preview)
//!   POST /api/workflows/{id}/test         (run bundled tests/ fixtures)
//!
//! See plan §3 (REST surface) + §4.5 (dry-run) + §7 (test fixtures).


use std::sync::Arc;

use aide::transform::TransformOperation;
use axum::extract::{Multipart, Path as AxumPath, Query};
use axum::http::StatusCode;
use axum::Json;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::checker::check_permission_union;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::with_permission;
use crate::modules::sync::{SyncAction, SyncOrigin};
use crate::modules::workflow::cost;
use crate::modules::workflow::models::{CreateWorkflow, CreateWorkflowRun, Workflow};
use crate::modules::workflow::permissions::{
    WorkflowsExecute, WorkflowsInstall, WorkflowsManage, WorkflowsRead,
};
use crate::modules::workflow::repository;
use crate::modules::workflow::test_runner::{
    self, FixtureMode, FixtureResult, TestFixture, TestRunResponse,
};
use crate::modules::workflow::validate;

// ============================================================
// POST /api/workflows/validate
// ============================================================

/// JSON body for `/validate` — just the workflow.yaml text. (A multipart
/// tarball would also work, but validate only needs the entry-point YAML
/// since it never installs.)
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ValidateWorkflowRequest {
    pub workflow_yaml: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidateErrorEntry {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidateWorkflowResponse {
    pub valid: bool,
    pub errors: Vec<ValidateErrorEntry>,
    pub warnings: Vec<ValidateErrorEntry>,
    pub steps: u64,
    pub est_max_calls: u64,
    pub est_max_tokens: u64,
}

pub async fn validate_workflow(
    _auth: RequirePermissions<(WorkflowsInstall,)>,
    Json(req): Json<ValidateWorkflowRequest>,
) -> ApiResult<Json<ValidateWorkflowResponse>> {
    // Layer 1 (shape) — parse. A parse failure is a single hard error.
    let parsed = match validate::parse_workflow_yaml(&req.workflow_yaml) {
        Ok(p) => p,
        Err(e) => {
            return Ok((
                StatusCode::OK,
                Json(ValidateWorkflowResponse {
                    valid: false,
                    errors: vec![ValidateErrorEntry {
                        code: "WORKFLOW_INVALID_YAML".into(),
                        location: None,
                        message: e.to_string(),
                    }],
                    warnings: vec![],
                    steps: 0,
                    est_max_calls: 0,
                    est_max_tokens: 0,
                }),
            ));
        }
    };

    // Layer 2 + 3 — validate, collecting ALL errors. `is_dev = true` so a
    // dev author's `mock:` fields don't trip the no-mock check at validate
    // time (validate is a dev affordance). FIX-4: prompt_file resolution uses a
    // guaranteed-nonexistent unique path (never created) instead of the shared
    // `/tmp` root, so a `WorkflowsInstall` caller can't probe real /tmp
    // contents. Since we don't have the bundle here, any `prompt_file:` is
    // reported as a (soft) missing-file error — acceptable for the YAML-only
    // validate surface.
    let tmp = std::env::temp_dir().join(format!("ziee-wf-validate-{}", Uuid::new_v4()));
    let raw = validate::validate_collecting(&parsed, &tmp, true);
    // Split findings by severity: errors gate `valid`; warnings (the
    // type-aware ref-check escape hatch for under-specified workflows) are
    // surfaced separately and never affect `valid`.
    let mut errors: Vec<ValidateErrorEntry> = Vec::new();
    let mut warnings: Vec<ValidateErrorEntry> = Vec::new();
    for e in raw {
        let entry = ValidateErrorEntry {
            code: e.code.to_string(),
            location: e.location,
            message: e.message,
        };
        match e.severity {
            validate::Severity::Error => errors.push(entry),
            validate::Severity::Warning => warnings.push(entry),
        }
    }

    let (steps, est_max_calls, est_max_tokens) = cost::estimate_static(&parsed);

    Ok((
        StatusCode::OK,
        Json(ValidateWorkflowResponse {
            valid: errors.is_empty(),
            errors,
            warnings,
            steps,
            est_max_calls,
            est_max_tokens,
        }),
    ))
}

pub fn validate_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsInstall,)>(op)
        .id("Workflow.validate")
        .tag("Workflows")
        .summary("Validate a workflow.yaml without installing")
        .description("Runs Layer 1+2+3 checks + static cost estimation. No DB row created.")
        .response::<200, Json<ValidateWorkflowResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

// ============================================================
// POST /api/workflows/import  (multipart, dev install)
// ============================================================

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct ImportQuery {
    /// Optional slug override; `local.dev/<slug>` becomes the row name.
    #[serde(default)]
    pub name: Option<String>,
    /// `user` (default) or `system`. `system` requires
    /// `workflows::manage_system`.
    #[serde(default)]
    pub scope: Option<String>,
}

pub async fn import_workflow(
    auth: RequirePermissions<(WorkflowsInstall,)>,
    Query(q): Query<ImportQuery>,
    origin: SyncOrigin,
    multipart: Multipart,
) -> ApiResult<Json<Workflow>> {
    import_workflow_inner(&auth.user, &auth.groups, q, origin, multipart).await
}

/// Shared multipart-import core. The user handler (`WorkflowsInstall`) and
/// the admin handler (`WorkflowsManageSystem`, system scope forced) both
/// delegate here so the extract→validate→install pipeline has a single
/// source of truth. `resolve_import_scope` re-checks `manage_system` for a
/// `system` request, so the user route can't escalate.
async fn import_workflow_inner(
    user: &crate::modules::user::models::User,
    groups: &[crate::modules::user::models::Group],
    q: ImportQuery,
    origin: SyncOrigin,
    multipart: Multipart,
) -> ApiResult<Json<Workflow>> {
    let upload = read_bundle_field(multipart).await?;
    let bytes = upload.bytes;
    // Query string takes precedence; fall back to the multipart name/scope
    // fields (the only path the frontend can use — see BundleUpload docs).
    let q = ImportQuery {
        name: q.name.or(upload.name),
        scope: q.scope.or(upload.scope),
    };
    install_workflow_from_bytes(user, groups, q, origin, bytes).await
}

/// Install a workflow from an in-memory tar.gz bundle — the shared
/// extract→validate→install core behind BOTH the multipart `import` handler
/// and the `workspace-save` promote path (which packs a conversation-workspace
/// dir into bytes). `q` must already carry the merged name/scope.
/// `resolve_import_scope` re-checks `manage_system`, so a user-scope caller
/// can never escalate to `system`.
pub(crate) async fn install_workflow_from_bytes(
    user: &crate::modules::user::models::User,
    groups: &[crate::modules::user::models::Group],
    q: ImportQuery,
    origin: SyncOrigin,
    bytes: Vec<u8>,
) -> ApiResult<Json<Workflow>> {
    // Scope resolution. `system` is admin-only.
    let scope = resolve_import_scope(user, groups, q.scope.as_deref(), "workflows")?;

    // Derive the dev slug + name.
    let slug = q
        .name
        .clone()
        .map(|s| sanitize_slug(&s))
        .unwrap_or_else(|| "imported-workflow".to_string());
    // H6: namespace the dev slug per user so user A's `local.dev/foo`
    // can't be clobbered by user B's import. System imports use the
    // `local.dev.system/` namespace.
    let owner_ns = if scope == "system" {
        "system".to_string()
    } else {
        user.id.to_string()
    };
    let name = format!("local.dev.{owner_ns}/{slug}");
    let version = "0.0.0-dev".to_string();

    // H1: owner-scope the on-disk dir too.
    let app_data_dir = crate::core::get_app_data_dir();
    let target_dir = app_data_dir
        .join("workflows")
        .join(&owner_ns)
        .join(&name)
        .join(&version);

    // Bomb-guarded extract (preserves execute bits for workflow scripts/).
    let extraction = crate::modules::hub::bundle::extract_tarball_bytes(
        &bytes,
        &target_dir,
        crate::modules::hub::bundle::BundleKind::Workflow,
    )
    .await?;

    // Parse + validate workflow.yaml. is_dev=true ALLOWS mock: fields.
    let entry_point = "workflow.yaml".to_string();
    let wf_path = extraction.extracted_path.join(&entry_point);
    let content = match tokio::fs::read_to_string(&wf_path).await {
        Ok(c) => c,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&extraction.extracted_path).await;
            return Err(AppError::bad_request(
                "WORKFLOW_NO_ENTRY_POINT",
                format!("bundle is missing workflow.yaml: {e}"),
            )
            .into());
        }
    };
    let workflow_def = match validate::parse_workflow_yaml(&content) {
        Ok(d) => d,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&extraction.extracted_path).await;
            return Err(e.into());
        }
    };
    if let Err(e) =
        validate::validate_for_install(&workflow_def, &extraction.extracted_path, true)
    {
        let _ = tokio::fs::remove_dir_all(&extraction.extracted_path).await;
        return Err(e.into());
    }

    // Reject if the computed MCP tool slug would overflow the 128-char
    // composed-name cap (slug body > 87 chars). Audit gap 4 / plan §4.
    if let Err(e) = crate::modules::workflow_mcp::tools::check_install_slug_len(&name) {
        let _ = tokio::fs::remove_dir_all(&extraction.extracted_path).await;
        return Err(e.into());
    }

    let owner_user_id = if scope == "system" {
        None
    } else {
        Some(user.id)
    };

    // Re-import overwrites: delete any prior row with the same
    // name+version (the extracted dir was already overwritten by
    // extract_tarball_bytes). H6: scope the pre-delete to THIS owner so
    // it can never delete another user's workflow row.
    if let Some(prior) = repository::find_by_name_version_owner(
        Repos.pool(),
        &name,
        Some(&version),
        owner_user_id,
    )
    .await?
    {
        repository::delete(Repos.pool(), prior.id).await?;
    }

    let create = CreateWorkflow {
        name: name.clone(),
        version: Some(version),
        display_name: Some(slug),
        description: workflow_def
            .steps
            .first()
            .map(|_| "Dev-imported workflow".to_string()),
        extracted_path: extraction.extracted_path.display().to_string(),
        bundle_sha256: extraction.sha256_hex.clone(),
        bundle_size_bytes: extraction.total_bytes as i64,
        file_count: extraction.file_count as i32,
        entry_point,
        tags: serde_json::Value::Array(vec![]),
        scope: scope.clone(),
        owner_user_id,
        created_by: Some(user.id),
        enabled: true,
        is_dev: true,
        // A promoted/imported workflow is permanent + listed, never ephemeral.
        ephemeral: false,
        conversation_id: None,
        // Pattern (d): compile the validated def into the typed IR so the
        // column is non-NULL + available to the runner (matches the hub
        // install path). See compiled.rs.
        compiled_ir_json: crate::modules::workflow::compiled::compile_to_json(&workflow_def),
    };

    let workflow = match repository::insert(Repos.pool(), create).await {
        Ok(w) => w,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&extraction.extracted_path).await;
            return Err(e.into());
        }
    };

    if scope == "system" {
        crate::modules::workflow::events::emit_system_workflow(
            SyncAction::Create,
            workflow.id,
            origin.0,
        );
    } else {
        crate::modules::workflow::events::emit_user_workflow(
            SyncAction::Create,
            workflow.id,
            user.id,
            origin.0,
        );
    }

    Ok((StatusCode::CREATED, Json(workflow)))
}

// ============================================================
// Visual builder: create / edit from a posted WorkflowDef
// ============================================================

/// Serialize a posted `WorkflowDef` into a one-file (`workflow.yaml`) tar.gz
/// bundle — the shared body behind the builder's create + edit-in-place paths.
/// Writes the YAML into a throwaway temp dir (cleaned on every exit path), then
/// packs it exactly as `workspace-save` packs a sandbox-authored dir, so the
/// downstream extract→validate→compile→insert core is byte-identical whether the
/// bundle came from a tarball upload or the builder. `serde_norway` (the repo's
/// maintained serde_yaml fork, used by `parse_workflow_yaml`) is the serializer.
pub(crate) async fn def_to_bundle_bytes(
    def: &validate::WorkflowDef,
) -> Result<Vec<u8>, AppError> {
    let yaml = serde_norway::to_string(def).map_err(|e| {
        AppError::bad_request(
            "WORKFLOW_SERIALIZE_FAILED",
            format!("failed to serialize workflow.yaml: {e}"),
        )
    })?;
    let tmp_dir = std::env::temp_dir().join(format!("ziee-wf-def-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&tmp_dir).await.map_err(|e| {
        AppError::internal_error(format!("failed to stage workflow bundle dir: {e}"))
    })?;
    // Build the bytes inside a scope so the temp dir is cleaned on BOTH the ok
    // and error paths (no RAII guard for a plain PathBuf, so do it explicitly).
    let result = async {
        let yaml_path = tmp_dir.join("workflow.yaml");
        tokio::fs::write(&yaml_path, yaml.as_bytes()).await.map_err(|e| {
            AppError::internal_error(format!("failed to write workflow.yaml: {e}"))
        })?;
        crate::modules::hub::bundle::pack_workspace_dir(&tmp_dir)
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    result
}

/// Optional query for the builder create endpoint.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct CreateWorkflowDefQuery {
    /// Optional slug override; `local.dev.<owner>/<slug>` becomes the row name.
    #[serde(default)]
    pub name: Option<String>,
}

/// `POST /api/workflows` — create a user-scope workflow from a posted
/// `WorkflowDef` (the visual builder's "save new"). Serializes the def to a
/// one-file bundle and runs the SAME extract→validate→compile→insert core as
/// tarball import (so validation + IR compilation are identical). Scope is
/// forced to `user` (never escalates to system through this route).
pub async fn create_user_workflow(
    auth: RequirePermissions<(WorkflowsInstall,)>,
    Query(q): Query<CreateWorkflowDefQuery>,
    origin: SyncOrigin,
    Json(def): Json<validate::WorkflowDef>,
) -> ApiResult<Json<Workflow>> {
    // FIX-2: reject a name collision up front. `install_workflow_from_bytes`'s
    // re-import path does delete+insert on a matching name+version, which would
    // silently delete a prior user workflow and orphan its `workflow_runs`.
    // Editing an existing workflow is the PUT /definition path — the create path
    // must never overwrite. Mirror the name/version/owner derivation
    // `install_workflow_from_bytes` performs for a user-scope import.
    let slug = q
        .name
        .clone()
        .map(|s| sanitize_slug(&s))
        .unwrap_or_else(|| "imported-workflow".to_string());
    let name = format!("local.dev.{}/{}", auth.user.id, slug);
    let version = "0.0.0-dev".to_string();
    if repository::find_by_name_version_owner(
        Repos.pool(),
        &name,
        Some(&version),
        Some(auth.user.id),
    )
    .await?
    .is_some()
    {
        return Err::<_, (StatusCode, AppError)>(
            AppError::new(
                StatusCode::CONFLICT,
                "WORKFLOW_NAME_EXISTS",
                format!("a workflow named '{slug}' already exists — choose a different name"),
            )
            .into(),
        );
    }

    let bytes = def_to_bundle_bytes(&def).await?;
    let iq = ImportQuery {
        name: q.name,
        scope: Some("user".to_string()),
    };
    install_workflow_from_bytes(&auth.user, &auth.groups, iq, origin, bytes).await
}

pub fn create_user_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsInstall,)>(op)
        .id("Workflow.create")
        .tag("Workflows")
        .summary("Create a user-scope workflow from a WorkflowDef (visual builder)")
        .description("Serializes the posted WorkflowDef to a workflow.yaml bundle and installs it as a user-scope workflow via the shared validate+compile+insert core. Scope is always 'user'.")
        .response::<201, Json<Workflow>>()
        .response_with::<400, (), _>(|r| r.description("Invalid definition"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<409, (), _>(|r| r.description("A workflow with this name already exists"))
}

/// `PUT /api/workflows/{id}/definition` — replace a user-scope workflow's
/// steps / inputs from a posted `WorkflowDef`, preserving the workflow id (so
/// run-history FKs survive). Owner-scoped (403 non-owner, 404 missing),
/// mirroring `update_user_workflow`.
///
/// Concurrency + atomicity: the whole operation is serialized per workflow id by
/// an IN-PROCESS per-id async mutex (FIX-1 — see `WORKFLOW_DEF_LOCKS`), and the
/// LIVE bundle at `extracted_path` is only ever mutated by an atomic
/// same-filesystem rename, so a failed write / measure / compile / swap / DB commit
/// can never leave the on-disk `workflow.yaml` and the row's sha/size/compiled_ir
/// describing different defs. (An in-process lock is the RIGHT boundary here: the
/// bundle it protects lives on NODE-LOCAL disk — `extracted_path` — so cross-node
/// DB locking would be wrong anyway.)
/// The flow (disk is the runner's source of truth, so it is swapped FIRST and the
/// DB metadata follows LAST — FIX-2):
///   1. Acquire the per-id async mutex guard (held for the whole critical section;
///      auto-released on drop — success, error, future-drop, OR panic), then
///      RE-READ the row under it.
///   2. Best-effort sweep this workflow's own orphaned `.staging-*`/`.old-*`
///      siblings, RECOVERING the live bundle from a `.old-*` if a prior update
///      crashed mid-swap (FIX-5 / FIX-2 recovery).
///   3. Validate FIRST against the existing, still-intact `extracted_path` (so
///      asset/`prompt_file:` refs resolve and no destructive op precedes a valid
///      def).
///   4. Copy the live bundle into a unique sibling staging dir (preserving every
///      sibling asset — `scripts/`, prompt files, `tests/`) and overwrite ONLY
///      `workflow.yaml` in the staging copy; measure + compile from staging. Any
///      failure here removes staging and returns — the live bundle + row untouched.
///   5. Atomically swap the staging dir into place (move live aside → move staging
///      in). A swap failure restores the live bundle (checked — FIX-3) and returns
///      a 500; it NEVER returns 200 on a swap/restore failure (FIX-2).
///   6. Commit the new metadata via `repository::update_definition` (NOT
///      delete+insert — that would change the id) LAST. A DB failure here returns a
///      500 (the on-disk def is already the new source of truth; a retry is safe).
///   7. The guard drops on every path; the map entry is pruned when no waiter holds it.
pub async fn update_user_workflow_definition(
    auth: RequirePermissions<(WorkflowsManage,)>,
    AxumPath(id): AxumPath<Uuid>,
    origin: SyncOrigin,
    Json(def): Json<validate::WorkflowDef>,
) -> ApiResult<Json<Workflow>> {
    // FIX-1: serialize concurrent `PUT /definition` on the SAME workflow id via an
    // IN-PROCESS per-id async mutex. All of re-read → validate → stage → measure →
    // compile → swap → update_definition runs while the guard is held, so two racers
    // can't interleave stage/commit/swap and leave the row's metadata describing one
    // def while on-disk workflow.yaml holds the other. The guard auto-releases on
    // Drop — success, every early-return / error, future-drop (client disconnect),
    // OR panic — so it can NEVER leak (unlike the previous pooled-connection Postgres
    // session advisory lock, whose manual unlock was skipped on future-drop/panic,
    // deadlocking every later update of this id until process restart). An in-process
    // lock is also the correct boundary: the bundle it guards lives on node-local
    // disk (`extracted_path`), so a cross-node DB lock would be wrong anyway.
    let lock_arc = workflow_def_lock(id);
    let _def_guard = lock_arc.lock().await;

    // Everything under the guard lives in this block; the guard held above covers all
    // of its early-returns.
    let result: ApiResult<Json<Workflow>> = async {
        // Re-read `existing` AFTER acquiring the lock so we operate on a consistent
        // snapshot (a racer that just committed is fully applied before we read).
        let existing = repository::find_by_id(Repos.pool(), id)
            .await?
            .ok_or_else(|| AppError::not_found("Workflow"))?;
        if existing.scope != "user" || existing.owner_user_id != Some(auth.user.id) {
            return Err::<_, (StatusCode, AppError)>(
                AppError::forbidden(
                    "WORKFLOW_FORBIDDEN",
                    "only the owner may edit a user-scope workflow",
                )
                .into(),
            );
        }

        let bundle_root = std::path::PathBuf::from(&existing.extracted_path);

        // FIX-5: best-effort sweep of orphaned `.staging-*` / `.old-*` siblings this
        // workflow left behind on a previous crash. Scoped to this workflow's own
        // bundle-name prefix; under the per-workflow lock no live op of THIS workflow
        // can own such a sibling, so any match is a dead orphan. Never fails the update.
        sweep_orphan_siblings(&bundle_root).await;

        // Step 1: validate FIRST, against the EXISTING (still-intact) bundle root, so
        // `prompt_file:` / sibling-asset refs still resolve — and, crucially, so NO
        // filesystem op happens before validation succeeds. On error the live bundle
        // AND the DB row are left untouched. is_dev=true so a builder-authored def
        // parses under the same relaxed rules as import.
        if let Err(e) = validate::validate_for_install(&def, &bundle_root, true) {
            return Err::<_, (StatusCode, AppError)>(e.into());
        }

        // Serialize the new workflow.yaml (same serializer `def_to_bundle_bytes`
        // uses — `serde_norway`). Purely in-memory; the live bundle is untouched.
        let yaml = serde_norway::to_string(&def).map_err(|e| {
            AppError::bad_request(
                "WORKFLOW_SERIALIZE_FAILED",
                format!("failed to serialize workflow.yaml: {e}"),
            )
        })?;

        // Step 2+3: build the new bundle in a UNIQUE SIBLING staging dir — copy the
        // live bundle (preserving `scripts/`, prompt files, `tests/`), overwrite ONLY
        // `workflow.yaml` in the copy, then measure + compile from the staging dir.
        // The live bundle is never mutated here, so ANY failure below just removes
        // staging and returns with the live bundle + row intact.
        let staging = sibling_with_suffix(&bundle_root, &format!(".staging-{}", Uuid::new_v4()));
        let build = async {
            crate::modules::workflow::runner::copy_dir_recursive(&bundle_root, &staging).await?;
            let staged_wf = staging.join(&existing.entry_point);
            tokio::fs::write(&staged_wf, yaml.as_bytes()).await.map_err(|e| {
                AppError::internal_error(format!("failed to write workflow.yaml: {e}"))
            })?;
            // Recompute bundle_sha256 / bundle_size_bytes / file_count from the staged
            // dir, reusing the install path's derivation (sha256 of the packed tar.gz +
            // decompressed byte total + regular-file count). Paths are root-relative,
            // so the metadata is identical whether measured on staging or the final
            // location.
            let measure = crate::modules::hub::bundle::pack_workspace_dir_measured(&staging)?;
            let compiled = crate::modules::workflow::compiled::compile_to_json(&def);
            Ok::<_, AppError>((measure, compiled))
        }
        .await;
        let (measure, compiled) = match build {
            Ok(v) => v,
            Err(e) => {
                // Pre-swap failure: clean up staging; live bundle + row untouched.
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err::<_, (StatusCode, AppError)>(e.into());
            }
        };

        // FIX-2: swap the DISK bundle FIRST (the runner re-parses on-disk yaml every
        // run, so disk is the source of truth), THEN commit metadata to the DB LAST.
        // Same-filesystem (sibling) renames are atomic. Any failure after staging
        // surfaces a real 500 — we NEVER return 200 unless disk AND DB are both
        // consistently updated.
        let old = sibling_with_suffix(&bundle_root, &format!(".old-{}", Uuid::new_v4()));
        if let Err(e) = tokio::fs::rename(&bundle_root, &old).await {
            // Couldn't move the live bundle aside — nothing changed on disk or in the DB.
            let _ = tokio::fs::remove_dir_all(&staging).await;
            tracing::error!(
                workflow_id = %id,
                live = %bundle_root.display(),
                error = %e,
                "workflow definition update: could not move live bundle aside; no change applied"
            );
            return Err::<_, (StatusCode, AppError)>(
                AppError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "WORKFLOW_UPDATE_SWAP_FAILED",
                    format!("could not stage the new workflow bundle: {e}"),
                )
                .into(),
            );
        }
        if let Err(e) = tokio::fs::rename(&staging, &bundle_root).await {
            // Live moved to `old` but staging couldn't take its place — restore the
            // live bundle so `extracted_path` is never left missing.
            // FIX-3: CHECK the restore; never claim a restore that didn't happen.
            match tokio::fs::rename(&old, &bundle_root).await {
                Ok(()) => {
                    let _ = tokio::fs::remove_dir_all(&staging).await;
                    tracing::error!(
                        workflow_id = %id,
                        live = %bundle_root.display(),
                        error = %e,
                        "workflow definition update: could not install the staged bundle; \
                         restored the previous live bundle. No change applied."
                    );
                    return Err::<_, (StatusCode, AppError)>(
                        AppError::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "WORKFLOW_UPDATE_SWAP_FAILED",
                            format!("could not install the new workflow bundle: {e}"),
                        )
                        .into(),
                    );
                }
                Err(re) => {
                    // Restore ALSO failed — the live bundle is now MISSING at extracted_path.
                    tracing::error!(
                        workflow_id = %id,
                        live = %bundle_root.display(),
                        old = %old.display(),
                        swap_error = %e,
                        restore_error = %re,
                        "workflow definition update: staged-bundle install FAILED and restore of \
                         the previous bundle ALSO failed; the live bundle is MISSING at \
                         extracted_path — recover it from the .old- sibling"
                    );
                    return Err::<_, (StatusCode, AppError)>(
                        AppError::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "WORKFLOW_UPDATE_BUNDLE_MISSING",
                            format!(
                                "workflow bundle could not be installed or restored; the live \
                                 bundle is missing (install error: {e}; restore error: {re})"
                            ),
                        )
                        .into(),
                    );
                }
            }
        }
        // Swap succeeded: the live bundle now holds the new def. Drop the old copy.
        let _ = tokio::fs::remove_dir_all(&old).await;

        // Step (last): commit the new metadata (DB). The on-disk def — the runner's
        // source of truth — is ALREADY the new one, so a DB failure here must NOT
        // report success: surface a 500 telling the client the bundle was updated on
        // disk but the metadata write failed (a retry is safe/idempotent — the same
        // def re-stages to identical bytes and re-commits).
        let updated = match repository::update_definition(
            Repos.pool(),
            id,
            &existing.extracted_path,
            &measure.sha256_hex,
            measure.total_bytes as i64,
            measure.file_count as i32,
            compiled,
        )
        .await
        {
            Ok(u) => u,
            Err(e) => {
                tracing::error!(
                    workflow_id = %id,
                    error = %e,
                    "workflow definition update: on-disk bundle was updated but the metadata DB \
                     write failed; the row still carries the OLD metadata (retry is safe/idempotent)"
                );
                return Err::<_, (StatusCode, AppError)>(
                    AppError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "WORKFLOW_UPDATE_METADATA_FAILED",
                        format!(
                            "workflow bundle was updated on disk but the metadata write failed \
                             (a retry is safe): {e}"
                        ),
                    )
                    .into(),
                );
            }
        };

        crate::modules::workflow::events::emit_user_workflow(
            SyncAction::Update,
            id,
            auth.user.id,
            origin.0,
        );
        Ok((StatusCode::OK, Json(updated)))
    }
    .await;

    // FIX-1: release the per-workflow lock (drop the guard) and then deterministically
    // prune the map entry so `WORKFLOW_DEF_LOCKS` can't grow unbounded (the
    // shared-concurrency-map rule). Drop our own `Arc` clone FIRST so the only
    // remaining strong ref is the map's own — then `remove_if` removes the entry iff
    // `strong_count == 1` (no other waiter still holds it). `remove_if` evaluates the
    // predicate while holding the DashMap shard lock, and a concurrent acquirer also
    // takes that shard lock via `entry(..)`, so the check-then-remove can't race a
    // waiter in: a waiter that already cloned keeps `strong_count >= 2` → entry kept.
    drop(_def_guard);
    drop(lock_arc);
    WORKFLOW_DEF_LOCKS.remove_if(&id, |_, arc| Arc::strong_count(arc) == 1);

    result
}

/// FIX-1: in-process per-workflow-id async locks serializing `PUT /definition`.
/// The bundle each guards lives on node-local disk (`extracted_path`), so this
/// process-local lock — not a cross-node DB lock — is the correct boundary. Each
/// `Arc<Mutex<()>>` is tiny (~16 B); the map is bounded by the deterministic
/// `remove_if` prune at the end of `update_user_workflow_definition`. Mirrors the
/// `CONVERSATION_LOCKS` / `FETCH_LOCKS` keyed-lock pattern in `code_sandbox`.
static WORKFLOW_DEF_LOCKS: Lazy<DashMap<Uuid, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

fn workflow_def_lock(id: Uuid) -> Arc<Mutex<()>> {
    WORKFLOW_DEF_LOCKS
        .entry(id)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// FIX-5 + FIX-2 recovery: reconcile this workflow's orphaned `.staging-*` /
/// `.old-*` sibling dirs left behind by a previously-crashed update of the workflow
/// whose bundle is `bundle_root`. SCOPED to that workflow's own bundle-name prefix —
/// other workflows share the parent (`workflows/`) dir and a concurrent update of one
/// of THEM may hold a live sibling, which must never be touched. This runs under the
/// per-workflow in-process lock, so no live op for THIS workflow is running and any
/// sibling matching this workflow's prefix is a dead orphan. Never returns an error
/// (a failed prune only logs) — it must not fail the update.
///
/// CRITICAL (FIX-2): a `.old-<uuid>` sibling is the ONLY surviving copy of the live
/// bundle if a prior update crashed BETWEEN the two swap renames (`live→.old` done,
/// `staging→live` NOT → `bundle_root` is missing and `.old-` holds the real bundle).
/// There is no boot reconciliation, so the sweep must RECOVER, never destroy: if
/// `bundle_root` is missing while a `.old-*` survives, PROMOTE the newest `.old-*`
/// back to `bundle_root` FIRST. Only once `bundle_root` exists (present all along or
/// just restored) may the remaining `.old-*` / `.staging-*` orphans be removed —
/// `bundle_root` is never left missing while a `.old-*` exists.
async fn sweep_orphan_siblings(bundle_root: &std::path::Path) {
    let (Some(parent), Some(base)) = (bundle_root.parent(), bundle_root.file_name()) else {
        return;
    };
    let base = base.to_string_lossy();
    let staging_prefix = format!("{base}.staging-");
    let old_prefix = format!("{base}.old-");
    let mut rd = match tokio::fs::read_dir(parent).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                parent = %parent.display(),
                error = %e,
                "workflow definition update: orphan sweep could not read the workflows dir"
            );
            return;
        }
    };

    // Collect the matching siblings first (so recovery can consider ALL `.old-*`
    // before anything is deleted). `.old-*` entries carry their mtime so the newest
    // (the most-recent interrupted swap) can be picked — the `<uuid>` suffix is random,
    // not time-ordered.
    let mut staging_dirs: Vec<std::path::PathBuf> = Vec::new();
    let mut old_dirs: Vec<(std::path::PathBuf, Option<std::time::SystemTime>)> = Vec::new();
    loop {
        match rd.next_entry().await {
            Ok(Some(entry)) => {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(&*staging_prefix) {
                    staging_dirs.push(entry.path());
                } else if name.starts_with(&*old_prefix) {
                    let mtime = entry.metadata().await.ok().and_then(|m| m.modified().ok());
                    old_dirs.push((entry.path(), mtime));
                }
            }
            Ok(None) => break,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "workflow definition update: orphan sweep directory iteration failed"
                );
                break;
            }
        }
    }

    // FIX-2: if the live bundle is MISSING but a `.old-*` survives, a prior update
    // crashed mid-swap and the newest `.old-*` is the sole surviving copy — restore it
    // BEFORE deleting anything.
    let live_missing = !tokio::fs::try_exists(bundle_root).await.unwrap_or(false);
    if live_missing && !old_dirs.is_empty() {
        // Newest by mtime (`None` sorts oldest, so a dir with a real mtime wins).
        if let Some(idx) = old_dirs
            .iter()
            .enumerate()
            .max_by_key(|(_, (_, mtime))| *mtime)
            .map(|(i, _)| i)
        {
            let (recovered, _) = old_dirs.remove(idx);
            match tokio::fs::rename(&recovered, bundle_root).await {
                Ok(()) => {
                    tracing::warn!(
                        recovered = %recovered.display(),
                        live = %bundle_root.display(),
                        "workflow definition update: recovered an interrupted update — \
                         promoted a surviving .old- bundle back to the live path"
                    );
                }
                Err(e) => {
                    // Restore failed → bundle_root is STILL missing. Do NOT delete any
                    // `.old-*` (they may hold the only copy). Staging dirs are never the
                    // live bundle, so they remain safe to prune.
                    tracing::warn!(
                        recovered = %recovered.display(),
                        live = %bundle_root.display(),
                        error = %e,
                        "workflow definition update: could not recover interrupted update; \
                         leaving .old- siblings in place (live bundle still missing)"
                    );
                    for path in staging_dirs {
                        remove_orphan_dir(&path).await;
                    }
                    return;
                }
            }
        }
    }

    // bundle_root now exists (present all along or just restored). Safe to remove the
    // remaining orphan `.staging-*` + `.old-*` siblings.
    for path in staging_dirs
        .into_iter()
        .chain(old_dirs.into_iter().map(|(p, _)| p))
    {
        remove_orphan_dir(&path).await;
    }
}

/// Best-effort `remove_dir_all` of a single orphan sibling dir, logging either way.
/// Never fails the caller.
async fn remove_orphan_dir(path: &std::path::Path) {
    if let Err(e) = tokio::fs::remove_dir_all(path).await {
        tracing::warn!(
            orphan = %path.display(),
            error = %e,
            "workflow definition update: could not remove orphan staging/old dir"
        );
    } else {
        tracing::info!(
            orphan = %path.display(),
            "workflow definition update: removed orphan staging/old dir"
        );
    }
}

/// Build a UNIQUE sibling path next to `dir` by appending `suffix` to its final
/// component (e.g. `<extracted_path>.staging-<uuid>`). A sibling shares `dir`'s
/// parent, so it lives on the SAME filesystem — the guarantee that makes the
/// atomic-swap renames in `update_user_workflow_definition` atomic.
fn sibling_with_suffix(dir: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let file_name = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let sibling_name = format!("{file_name}{suffix}");
    match dir.parent() {
        Some(parent) => parent.join(sibling_name),
        None => std::path::PathBuf::from(sibling_name),
    }
}

pub fn update_user_workflow_definition_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsManage,)>(op)
        .id("Workflow.updateDefinition")
        .tag("Workflows")
        .summary("Replace a user-scope workflow's steps/inputs in place")
        .description("Re-materializes the workflow bundle from the posted WorkflowDef, preserving the workflow id (run-history FKs survive). Owner-scoped: 403 for a non-owner, 404 for a missing workflow.")
        .response::<200, Json<Workflow>>()
        .response_with::<400, (), _>(|r| r.description("Invalid definition"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Not the owner"))
        .response_with::<404, (), _>(|r| r.description("Workflow not found"))
}

// ============================================================
// POST /api/workflows/validate-def  (validate a posted WorkflowDef)
// ============================================================

/// Validate a posted `WorkflowDef` JSON (the builder's live-validation feed) —
/// the JSON-body twin of `/validate` (which takes workflow.yaml text). Returns
/// all findings + a dry-run cost estimate as a 200 (validation findings are
/// structured data, never a hard 4xx). Uses a throwaway temp dir as the bundle
/// root (same as the YAML `/validate` surface).
pub async fn validate_workflow_def(
    _auth: RequirePermissions<(WorkflowsRead,)>,
    Json(def): Json<validate::WorkflowDef>,
) -> ApiResult<Json<crate::modules::workflow::models::ValidateDefResponse>> {
    // FIX-4: a guaranteed-nonexistent unique path (never created) as the bundle
    // root, so `prompt_file:` resolution can't stat real shared-/tmp contents
    // (a `WorkflowsRead` caller must not be able to probe path existence).
    let tmp = std::env::temp_dir().join(format!("ziee-wf-validate-{}", Uuid::new_v4()));
    let raw = validate::validate_collecting(&def, &tmp, true);
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for e in raw {
        match e.severity {
            validate::Severity::Error => errors.push(e),
            validate::Severity::Warning => warnings.push(e),
        }
    }
    let cost_estimate = cost::dry_run(&def, &serde_json::Map::new());
    Ok((
        StatusCode::OK,
        Json(crate::modules::workflow::models::ValidateDefResponse {
            errors,
            warnings,
            cost_estimate,
        }),
    ))
}

pub fn validate_workflow_def_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.validateDef")
        .tag("Workflows")
        .summary("Validate a WorkflowDef JSON (no install)")
        .description("Runs the semantic + security checks + a dry-run cost estimate on a posted WorkflowDef. Findings are returned structured with a 200 (they never hard-fail the request).")
        .response::<200, Json<crate::modules::workflow::models::ValidateDefResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

pub fn import_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsInstall,)>(op)
        .id("Workflow.import")
        .tag("Workflows")
        .summary("Dev-import a workflow bundle (multipart tarball)")
        .description("Extract a tar.gz of the workflow source dir, validate (is_dev), install as local.dev/<slug>. Re-import overwrites.")
        .response::<201, Json<Workflow>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden (system scope without admin)"))
}

// ============================================================
// POST /api/workflows/system/import  (admin multipart, system scope)
// ============================================================

/// Admin-scope multipart dev-import. Forces `scope=system` regardless of
/// the query param and is gated on `WorkflowsManageSystem` (vs the user
/// import's `WorkflowsInstall`). Delegates to the shared `import_workflow`
/// body with the scope overridden to `system`. Mirrors the skills surface
/// intent — the user `/import` IS the create path, so this is the admin
/// equivalent (there is no plain hand-authored create endpoint; see the
/// routes.rs comment on the create-vs-import decision).
pub async fn import_system_workflow(
    auth: RequirePermissions<(crate::modules::workflow::permissions::WorkflowsManageSystem,)>,
    Query(mut q): Query<ImportQuery>,
    origin: SyncOrigin,
    multipart: Multipart,
) -> ApiResult<Json<Workflow>> {
    // Force system scope regardless of any client-supplied query param.
    q.scope = Some("system".to_string());
    // Delegate to the shared core. The admin already holds manage_system;
    // `resolve_import_scope` re-checks it for the system scope, so this is
    // belt-and-suspenders, not a privilege grant.
    import_workflow_inner(&auth.user, &auth.groups, q, origin, multipart).await
}

pub fn import_system_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(crate::modules::workflow::permissions::WorkflowsManageSystem,)>(op)
        .id("Workflow.importSystem")
        .tag("Workflows - Admin")
        .summary("Admin dev-import a SYSTEM-WIDE workflow bundle (multipart tarball)")
        .description("Same extract+validate pipeline as the user import, but installs as scope='system' and requires workflows::manage_system.")
        .response::<201, Json<Workflow>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden (requires workflows::manage_system)"))
}

// ============================================================
// POST /api/workflows/{id}/dry-run  (cost preview)
// ============================================================

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct DryRunRequest {
    #[serde(default)]
    pub inputs: serde_json::Value,
}

pub async fn dry_run(
    auth: RequirePermissions<(WorkflowsExecute,)>,
    AxumPath(id): AxumPath<Uuid>,
    Json(req): Json<DryRunRequest>,
) -> ApiResult<Json<cost::DryRunResult>> {
    // H-2: gate exactly like get/run — `user_can_access` enforces both
    // ownership (user-scope) AND group-restriction (system-scope). The old
    // `scope == "user"` check skipped the group check for system workflows,
    // letting a non-member dry-run a group-restricted workflow they can't see.
    if !repository::user_can_access(Repos.pool(), auth.user.id, id).await? {
        return Err::<_, (StatusCode, AppError)>(
            AppError::not_found("Workflow").into(),
        );
    }
    let wf = repository::find_by_id(Repos.pool(), id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;

    let wf_path = std::path::PathBuf::from(&wf.extracted_path).join(&wf.entry_point);
    let content = tokio::fs::read_to_string(&wf_path).await.map_err(|e| {
        AppError::internal_error(format!("dry-run: read workflow.yaml: {e}"))
    })?;
    let workflow_def = validate::parse_workflow_yaml(&content)?;

    // Validate + bind inputs against workflow.inputs[].
    let bound = bind_inputs(&workflow_def, req.inputs)?;

    let result = cost::dry_run(&workflow_def, &bound);
    Ok((StatusCode::OK, Json(result)))
}

pub fn dry_run_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsExecute,)>(op)
        .id("Workflow.dryRun")
        .tag("Workflows")
        .summary("Cost preview — walk the DAG without executing")
        .description("Per-step est_calls + est_tokens; llm_map fan-out marked runtime-dependent when for_each refs a prior step. Zero tokens spent, no run row.")
        .response::<200, Json<cost::DryRunResult>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

// ============================================================
// POST /api/workflows/{id}/test  (run bundled fixtures)
// ============================================================

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct TestWorkflowRequest {
    /// Conversation to snapshot the model from (mirrors /run). Required
    /// for `real_llm` fixtures; ci fixtures are fully mocked but still
    /// need a model snapshot to build the (never-called) provider object.
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
}

pub async fn test_workflow(
    auth: RequirePermissions<(WorkflowsExecute,)>,
    AxumPath(id): AxumPath<Uuid>,
    Json(req): Json<TestWorkflowRequest>,
) -> ApiResult<Json<TestRunResponse>> {
    let pool = Repos.pool().clone();
    // H-2: same access gate as get/run/dry_run (ownership + group restriction).
    if !repository::user_can_access(&pool, auth.user.id, id).await? {
        return Err::<_, (StatusCode, AppError)>(
            AppError::not_found("Workflow").into(),
        );
    }
    let wf = repository::find_by_id(&pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;

    // Parse the on-disk workflow.yaml.
    let wf_path = std::path::PathBuf::from(&wf.extracted_path).join(&wf.entry_point);
    let content = tokio::fs::read_to_string(&wf_path).await.map_err(|e| {
        AppError::internal_error(format!("test: read workflow.yaml: {e}"))
    })?;
    let workflow_def = validate::parse_workflow_yaml(&content)?;

    // Load fixtures from <extracted_path>/tests/*.yaml.
    let fixtures = load_fixtures(&wf.extracted_path).await?;
    if fixtures.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(TestRunResponse {
                total: 0,
                passed: 0,
                failed: 0,
                skipped: 0,
                results: vec![],
            }),
        ));
    }

    // Resolve a model snapshot + provider once (shared across fixtures).
    // For ci-mode fixtures the provider is never invoked (all steps
    // mocked); for real_llm it's the real path.
    let model_provider = resolve_test_model(&wf, &req, auth.user.id).await;

    let mut results: Vec<FixtureResult> = Vec::with_capacity(fixtures.len());
    for (name, fixture) in fixtures {
        let started = std::time::Instant::now();
        let res = run_one_fixture(
            &pool,
            &wf,
            &workflow_def,
            &name,
            fixture,
            &model_provider,
            req.conversation_id,
            auth.user.id,
            started,
        )
        .await;
        results.push(res);
    }

    let passed = results.iter().filter(|r| r.passed && !r.skipped).count() as u32;
    let skipped = results.iter().filter(|r| r.skipped).count() as u32;
    let failed = results.iter().filter(|r| !r.passed && !r.skipped).count() as u32;

    Ok((
        StatusCode::OK,
        Json(TestRunResponse {
            total: results.len() as u32,
            passed,
            failed,
            skipped,
            results,
        }),
    ))
}

pub fn test_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsExecute,)>(op)
        .id("Workflow.test")
        .tag("Workflows")
        .summary("Run bundled tests/<name>.yaml fixtures")
        .description("mode: ci requires mocks covering all llm/llm_map steps; mocks honored regardless of is_dev (the sanctioned mock context). Assertion modes: contains / equals / min_length / max_length / matches_schema.")
        .response::<200, Json<TestRunResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

// ============================================================
// Helpers
// ============================================================

/// Pull the `bundle` form field out of a multipart upload as raw bytes.
/// The bundle bytes plus the optional `name`/`scope` text fields. The
/// frontend sends `name`/`scope` as multipart fields (the generated API
/// client cannot attach query params to a FormData POST — `core.ts` only
/// appends query params on GET), so the handler reads them here and falls
/// back to them when the query string is empty.
struct BundleUpload {
    bytes: Vec<u8>,
    name: Option<String>,
    scope: Option<String>,
}

async fn read_bundle_field(mut multipart: Multipart) -> Result<BundleUpload, AppError> {
    let mut bytes: Option<Vec<u8>> = None;
    let mut name: Option<String> = None;
    let mut scope: Option<String> = None;
    while let Ok(Some(mut field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "bundle" => {
                // L10: stream chunk-by-chunk with a hard cap rather than
                // `field.bytes()` (unbounded RAM buffering). Bound the upload
                // at the bundle compressed cap on this authenticated endpoint.
                let cap = crate::modules::hub::bundle::MAX_BUNDLE_COMPRESSED_BYTES as usize;
                let mut data: Vec<u8> = Vec::new();
                while let Some(chunk) = field.chunk().await.map_err(|e| {
                    AppError::bad_request("IMPORT_READ_FAILED", format!("read bundle field: {e}"))
                })? {
                    if data.len().saturating_add(chunk.len()) > cap {
                        return Err(AppError::unprocessable_entity(
                            "IMPORT_BUNDLE_TOO_LARGE",
                            format!("uploaded bundle exceeds the {cap}-byte cap"),
                        ));
                    }
                    data.extend_from_slice(&chunk);
                }
                bytes = Some(data);
            }
            "name" => {
                name = field.text().await.ok().filter(|s| !s.is_empty());
            }
            "scope" => {
                scope = field.text().await.ok().filter(|s| !s.is_empty());
            }
            _ => {}
        }
    }
    let bytes = bytes.ok_or_else(|| {
        AppError::bad_request(
            "IMPORT_NO_BUNDLE",
            "multipart upload missing a `bundle` form field carrying the tar.gz",
        )
    })?;
    Ok(BundleUpload { bytes, name, scope })
}

/// Resolve the requested scope for a dev import. `system` requires the
/// `<module>::manage_system` permission; everything else is `user`.
pub(crate) fn resolve_import_scope(
    user: &crate::modules::user::models::User,
    groups: &[crate::modules::user::models::Group],
    requested: Option<&str>,
    module: &str,
) -> Result<String, AppError> {
    match requested {
        Some("system") => {
            let perm = format!("{module}::manage_system");
            if user.is_admin || check_permission_union(user, groups, &perm) {
                Ok("system".to_string())
            } else {
                Err(AppError::forbidden(
                    "IMPORT_SYSTEM_FORBIDDEN",
                    format!("system-scope import requires {perm}"),
                ))
            }
        }
        _ => Ok("user".to_string()),
    }
}

/// `^[a-z0-9._-]+$`-safe slug from arbitrary input.
fn sanitize_slug(raw: &str) -> String {
    let s: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    // Reject a dot-only slug (".", "..", "…") so it can't act as a path
    // component when joined into the extracted-bundle dir.
    if s.is_empty() || s.chars().all(|c| c == '.') {
        "imported".to_string()
    } else {
        s
    }
}

/// Bind request inputs against workflow.inputs[] (defaults + required).
fn bind_inputs(
    workflow: &validate::WorkflowDef,
    inputs: serde_json::Value,
) -> Result<serde_json::Map<String, serde_json::Value>, AppError> {
    let provided = match inputs {
        serde_json::Value::Object(m) => m,
        serde_json::Value::Null => Default::default(),
        _ => {
            return Err(AppError::bad_request(
                "WORKFLOW_INPUTS_NOT_OBJECT",
                "inputs must be a JSON object",
            ));
        }
    };
    let mut bound = serde_json::Map::new();
    for input in &workflow.inputs {
        if let Some(v) = provided.get(&input.name) {
            bound.insert(input.name.clone(), v.clone());
        } else if let Some(d) = &input.default {
            bound.insert(input.name.clone(), d.clone());
        } else if input.required {
            return Err(AppError::bad_request(
                "WORKFLOW_INPUT_MISSING",
                format!("required input '{}' not provided", input.name),
            ));
        }
    }
    Ok(bound)
}

/// Read every `tests/*.yaml` (or `.yml`) under the extracted bundle.
async fn load_fixtures(extracted_path: &str) -> Result<Vec<(String, TestFixture)>, AppError> {
    let dir = std::path::PathBuf::from(extracted_path).join("tests");
    let mut out = Vec::new();
    let mut rd = match tokio::fs::read_dir(&dir).await {
        Ok(rd) => rd,
        Err(_) => return Ok(out), // no tests/ dir → empty (no error)
    };
    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let is_yaml = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false);
        if !is_yaml {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("fixture")
            .to_string();
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            AppError::internal_error(format!("test: read fixture {}: {e}", path.display()))
        })?;
        let fixture: TestFixture = serde_norway::from_str(&content).map_err(|e| {
            AppError::bad_request(
                "WORKFLOW_FIXTURE_INVALID",
                format!("tests/{name}.yaml is malformed: {e}"),
            )
        })?;
        out.push((name, fixture));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Snapshot a model + build a provider for test runs. Returns None when
/// no model is resolvable (real_llm fixtures then report skipped).
async fn resolve_test_model(
    wf: &Workflow,
    req: &TestWorkflowRequest,
    user_id: Uuid,
) -> Option<(Uuid, String, std::sync::Arc<ai_providers::Provider>)> {
    let _ = wf;
    let conv_id = req.conversation_id?;
    let conv = crate::core::Repos
        .chat
        .core
        .get_conversation(conv_id, user_id)
        .await
        .ok()
        .flatten()?;
    let model_id = conv.model_id?;
    let model = crate::core::Repos.llm_model.get_by_id(model_id).await.ok().flatten()?;
    let (provider, _name, _mid, _pid, _params, _caps) =
        crate::modules::chat::core::ai_provider::create_provider_from_model_id(model_id, user_id)
            .await
            .ok()?;
    Some((model_id, model.name, provider))
}

#[allow(clippy::too_many_arguments)]
async fn run_one_fixture(
    pool: &sqlx::PgPool,
    wf: &Workflow,
    workflow_def: &validate::WorkflowDef,
    name: &str,
    fixture: TestFixture,
    model_provider: &Option<(Uuid, String, std::sync::Arc<ai_providers::Provider>)>,
    conversation_id: Option<Uuid>,
    user_id: Uuid,
    started: std::time::Instant,
) -> FixtureResult {
    let fail = |output_name: &str, assertion: &str, expected: String, actual: String| {
        FixtureResult {
            name: name.to_string(),
            passed: false,
            skipped: false,
            duration_ms: started.elapsed().as_millis() as u64,
            failure: Some(test_runner::FixtureFailure {
                output_name: output_name.to_string(),
                assertion: assertion.to_string(),
                expected,
                actual_preview: actual,
            }),
        }
    };

    // ci mode: every llm/llm_map step MUST be mocked.
    if fixture.mode == FixtureMode::Ci {
        let missing = test_runner::missing_mock_steps(workflow_def, &fixture.mocks);
        if !missing.is_empty() {
            return fail(
                "",
                "missing_mocks",
                format!("mocks covering steps: {}", missing.join(", ")),
                format!("un-mocked: {}", missing.join(", ")),
            );
        }
    }

    // Resolve model + provider. Required for both modes (ci uses it only
    // to satisfy the runner's type; real_llm actually calls it).
    let (model_id, model_name, provider) = match model_provider {
        Some(mp) => mp.clone(),
        None => {
            // real_llm with no provider → skipped (not failed); ci with no
            // model is also skipped because we can't construct the runner.
            return FixtureResult {
                name: name.to_string(),
                passed: false,
                skipped: true,
                duration_ms: started.elapsed().as_millis() as u64,
                failure: Some(test_runner::FixtureFailure {
                    output_name: "".into(),
                    assertion: "skipped_no_model".into(),
                    expected: "a resolvable model (pass conversation_id)".into(),
                    actual_preview: "no model available".into(),
                }),
            };
        }
    };

    let sandbox_flavor = workflow_def.sandbox.as_ref().map(|s| s.flavor.clone());
    let run_row = match repository::insert_run(
        pool,
        CreateWorkflowRun {
            workflow_id: wf.id,
            conversation_id,
            user_id,
            model_id: Some(model_id),
            sandbox_flavor,
            run_kind: "test".into(),
            invocation_source: "manual".into(),
            inputs_json: fixture.inputs.clone(),
        },
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return fail("", "run_setup", "a workflow_runs row".into(), e.to_string());
        }
    };

    let outcome = crate::modules::workflow::runner::run_for_test(
        pool,
        run_row.id,
        user_id,
        conversation_id,
        wf,
        workflow_def,
        fixture.inputs.clone(),
        fixture.mocks.clone(),
        model_id,
        model_name,
        provider,
    )
    .await;

    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => return fail("", "run_failed", "a completed run".into(), e.to_string()),
    };

    if outcome.status != crate::modules::workflow::models::WorkflowRunStatus::Completed {
        let err = outcome.error.unwrap_or_else(|| "run did not complete".into());
        // real_llm runs that fail because no provider is configured are
        // reported skipped, not failed (plan §3 + §7).
        if fixture.mode == FixtureMode::RealLlm
            && (err.contains("provider") || err.contains("PROVIDER") || err.contains("api_key"))
        {
            return FixtureResult {
                name: name.to_string(),
                passed: false,
                skipped: true,
                duration_ms: started.elapsed().as_millis() as u64,
                failure: Some(test_runner::FixtureFailure {
                    output_name: "".into(),
                    assertion: "skipped_no_provider".into(),
                    expected: "a configured provider".into(),
                    actual_preview: err,
                }),
            };
        }
        return fail("", "run_failed", "a completed run".into(), err);
    }

    // Compare resolved outputs against expected_outputs.
    for (output_name, assertion_set) in &fixture.expected_outputs {
        let actual = match outcome.outputs.get(output_name) {
            Some(v) => v,
            None => {
                return fail(
                    output_name,
                    "output_present",
                    "an output named this".into(),
                    "output not produced by the run".into(),
                );
            }
        };
        if let Err(f) = test_runner::check_assertions(output_name, assertion_set, actual) {
            return FixtureResult {
                name: name.to_string(),
                passed: false,
                skipped: false,
                duration_ms: started.elapsed().as_millis() as u64,
                failure: Some(f),
            };
        }
    }

    FixtureResult {
        name: name.to_string(),
        passed: true,
        skipped: false,
        duration_ms: started.elapsed().as_millis() as u64,
        failure: None,
    }
}

// ============================================================
// POST /api/workflows/workspace-save   (promote an LLM-authored bundle)
// GET  /api/workflows/workspace-export  (download it as tar.gz)
// ============================================================

/// Promote a workflow the model authored in its sandbox workspace into the
/// user's permanent library. `scope="system"` is admin-only (re-checked by
/// `resolve_import_scope` inside `install_workflow_from_bytes`).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WorkspaceSaveRequest {
    /// The conversation whose sandbox workspace holds the bundle.
    pub conversation_id: Uuid,
    /// The workspace subdir (relative to the conversation workspace) with
    /// `workflow.yaml` (+ any `scripts/`).
    pub dir: String,
    /// Optional slug/name for the saved workflow.
    #[serde(default)]
    pub name: Option<String>,
    /// `user` (default) or `system` (admin-only).
    #[serde(default)]
    pub scope: Option<String>,
}

pub async fn workspace_save(
    auth: RequirePermissions<(WorkflowsInstall,)>,
    origin: SyncOrigin,
    Json(req): Json<WorkspaceSaveRequest>,
) -> ApiResult<Json<Workflow>> {
    // Ownership gate: the conversation must belong to the caller (else this
    // could pack + install another user's workspace files).
    crate::modules::workflow::workspace::require_conversation_owner(
        Some(req.conversation_id),
        auth.user.id,
    )
    .await?;
    let root = crate::modules::workflow::workspace::resolve_conversation_workspace_dir(
        Some(req.conversation_id),
        &req.dir,
    )?;
    let bytes = crate::modules::hub::bundle::pack_workspace_dir(&root)?;
    let q = ImportQuery {
        name: req.name,
        scope: req.scope,
    };
    install_workflow_from_bytes(&auth.user, &auth.groups, q, origin, bytes).await
}

pub fn workspace_save_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsInstall,)>(op)
        .id("Workflow.workspaceSave")
        .tag("Workflows")
        .summary("Save a sandbox-authored workflow into the library")
        .description(
            "Packs the conversation-workspace <dir> bundle and installs it as a \
             permanent workflow (scope=user, or scope=system for admins).",
        )
        .response::<201, Json<Workflow>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden (system scope needs admin)"))
        .response_with::<400, (), _>(|r| r.description("Invalid dir or bundle"))
}

/// Query for `GET /api/workflows/workspace-export`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WorkspaceExportQuery {
    pub conversation_id: Uuid,
    pub dir: String,
}

pub async fn workspace_export(
    auth: RequirePermissions<(WorkflowsExecute,)>,
    Query(q): Query<WorkspaceExportQuery>,
) -> ApiResult<axum::response::Response> {
    // Ownership gate: the conversation must belong to the caller (else this
    // could export another user's workspace files).
    crate::modules::workflow::workspace::require_conversation_owner(
        Some(q.conversation_id),
        auth.user.id,
    )
    .await?;
    let root = crate::modules::workflow::workspace::resolve_conversation_workspace_dir(
        Some(q.conversation_id),
        &q.dir,
    )?;
    let bytes = crate::modules::hub::bundle::pack_workspace_dir(&root)?;
    // Filename from the leaf dir component (sanitized), else a default.
    let leaf = root
        .file_name()
        .and_then(|s| s.to_str())
        .map(sanitize_slug)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "workflow".to_string());
    let filename = format!("{leaf}.tar.gz");
    let len = bytes.len();
    let resp = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/gzip")
        .header(axum::http::header::CONTENT_LENGTH, len.to_string())
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(bytes))
        .map_err(|e| AppError::internal_error(format!("response: {e}")))?;
    Ok((StatusCode::OK, resp))
}

pub fn workspace_export_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsExecute,)>(op)
        .id("Workflow.workspaceExport")
        .tag("Workflows")
        .summary("Download a sandbox-authored workflow bundle as tar.gz")
        .response_with::<200, (), _>(|r| r.description("The workflow bundle (application/gzip)"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<400, (), _>(|r| r.description("Invalid dir or bundle"))
}
