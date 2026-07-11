//! Admin REST handlers for the whisper-MODEL library: catalog listing, download
//! (async SSE), upload, and the installed set (list / delete / activate).
//!
//! Gated by the voice admin split (`voice::admin::{read,manage}`), mirroring
//! `runtime_version::handlers` (the engine-BINARY surface).

use aide::axum::IntoApiResponse;
use axum::{
    Json,
    extract::{Multipart, Path, Query},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::permissions::{RequirePermissions, with_permission},
    modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
};

use super::handlers::is_valid_model_name;
use super::model;
use super::model_catalog;
use super::model_download_task::{
    self, MODEL_DOWNLOAD_TASKS, ModelDownloadTask, SSEModelDownloadConnectedData,
    SSEModelDownloadEvent,
};
use super::models::*;
use super::permissions::{VoiceAdminManage, VoiceAdminRead};
use super::repository::VoiceModelRow;

// ─────────────────────────────── helpers ─────────────────────────────────

/// A safe HF-repo-relative filename for the fetch URL: non-empty, no absolute
/// path, no `..` traversal segment, reasonable length + charset.
fn is_safe_remote_filename(f: &str) -> bool {
    !f.is_empty()
        && f.len() <= 200
        && !f.starts_with('/')
        && !f.split('/').any(|seg| seg == ".." || seg == ".")
        && f.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
}


/// Project a DB row into the API `VoiceModel`, marking active + update-available.
fn to_api(
    row: VoiceModelRow,
    active_model: &str,
    catalog_sha: Option<&std::collections::HashMap<String, String>>,
) -> VoiceModel {
    let is_active = row.name == active_model;
    // update_available: the upstream catalog now advertises a different oid than
    // the recorded digest for this model's filename.
    let update_available = match (catalog_sha, row.sha256.as_deref()) {
        (Some(map), Some(local)) => map
            .get(&row.filename)
            .map(|up| !up.eq_ignore_ascii_case(local))
            .unwrap_or(false),
        _ => false,
    };
    VoiceModel {
        id: row.id,
        name: row.name,
        filename: row.filename,
        source: row.source,
        source_url: row.source_url,
        size_bytes: row.size_bytes,
        sha256: row.sha256,
        verified: row.verified,
        is_active,
        update_available,
        created_at: row.created_at,
    }
}

// ─────────────────────────────── catalog ─────────────────────────────────

/// List downloadable models from the configured source (runtime fetch). Degrades
/// gracefully: an unreachable source yields an empty list + `source_reachable:false`.
pub async fn get_catalog(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<VoiceCatalogResponse>> {
    let settings = Repos.voice.get_settings().await?;
    let installed = Repos.voice_model.list().await?;
    let installed_names: std::collections::HashSet<String> =
        installed.iter().map(|m| m.name.clone()).collect();

    let (entries, reachable) = model_catalog::fetch_catalog(&settings.model_source_repo).await;
    let models = entries
        .into_iter()
        .map(|e| VoiceCatalogModel {
            installed: installed_names.contains(&e.name),
            name: e.name,
            filename: e.filename,
            size_bytes: e.size_bytes,
            sha256: e.sha256,
            english_only: e.english_only,
            quantization: e.quantization,
        })
        .collect();
    Ok((
        StatusCode::OK,
        Json(VoiceCatalogResponse {
            models,
            source_reachable: reachable,
            source_repo: settings.model_source_repo,
        }),
    ))
}

pub fn get_catalog_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.listModelCatalog")
        .tag("Voice")
        .summary("List downloadable whisper models from the configured source")
        .response::<200, Json<VoiceCatalogResponse>>()
}

// ─────────────────────────── installed set ───────────────────────────────

/// List installed models (with active + update-available flags).
pub async fn list_models(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<Vec<VoiceModel>>> {
    let settings = Repos.voice.get_settings().await?;
    let rows = Repos.voice_model.list().await?;
    // Best-effort catalog oids for update-detection (never fails the list).
    let (entries, _) = model_catalog::fetch_catalog(&settings.model_source_repo).await;
    let catalog_sha: std::collections::HashMap<String, String> = entries
        .into_iter()
        .filter_map(|e| e.sha256.map(|s| (e.filename, s)))
        .collect();
    let models = rows
        .into_iter()
        .map(|r| to_api(r, &settings.model, Some(&catalog_sha)))
        .collect();
    Ok((StatusCode::OK, Json(models)))
}

pub fn list_models_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.listModels")
        .tag("Voice")
        .summary("List installed whisper models")
        .response::<200, Json<Vec<VoiceModel>>>()
}

/// Set the active model (updates `settings.model`; the auto-start path drains +
/// respawns the whisper-server on the next request / immediately if running).
pub async fn activate_model(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<Json<VoiceModel>> {
    let row = Repos
        .voice_model
        .get_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("voice model"))?;

    Repos
        .voice
        .update_settings(
            None,
            Some(row.name.clone()),
            None, None, None, None, None, None, None, None, None, None,
        )
        .await?;

    // Drain + respawn if a whisper-server is currently running on another model.
    super::auto_start::apply_active_model_change().await;

    sync_publish(
        SyncEntity::VoiceModel,
        SyncAction::Update,
        row.id,
        Audience::perm::<VoiceAdminRead>(),
        origin.0.clone(),
    );
    sync_publish(
        SyncEntity::VoiceSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<VoiceAdminRead>(),
        origin.0,
    );
    let api = to_api(row, "", None);
    Ok((StatusCode::OK, Json(VoiceModel { is_active: true, ..api })))
}

pub fn activate_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.activateModel")
        .tag("Voice")
        .summary("Set a model as the active whisper model")
        .response::<200, Json<VoiceModel>>()
}

/// Delete an installed model (row + on-disk file). Refuses (409) to delete the
/// currently-active model unless `?ack_active=true`.
pub async fn delete_model(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    Path(id): Path<Uuid>,
    Query(q): Query<DeleteModelQuery>,
    origin: SyncOrigin,
) -> ApiResult<impl IntoApiResponse> {
    let row = match Repos.voice_model.get_by_id(id).await? {
        Some(r) => r,
        None => return Ok((StatusCode::NO_CONTENT, ())),
    };
    let settings = Repos.voice.get_settings().await?;
    if settings.model == row.name && !q.ack_active {
        return Err(AppError::conflict(
            "cannot delete the active model without ack_active=true (activate another model first)",
        )
        .to_api_error());
    }

    // Remove the on-disk file (best-effort) then the row.
    let path = model::models_dir().join(&row.filename);
    let _ = std::fs::remove_file(&path);
    Repos.voice_model.delete(id).await?;

    sync_publish(
        SyncEntity::VoiceModel,
        SyncAction::Delete,
        row.id,
        Audience::perm::<VoiceAdminRead>(),
        origin.0,
    );
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.deleteModel")
        .tag("Voice")
        .summary("Delete an installed whisper model")
        .response::<204, ()>()
}

// ─────────────────────────────── download ────────────────────────────────

/// Start (or join) a detached model download. Resolves the source (catalog /
/// HF-repo / arbitrary URL) into a [`model::ModelDownloadSpec`].
pub async fn download_model(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    Json(req): Json<DownloadModelRequest>,
) -> ApiResult<Json<DownloadModelStartedResponse>> {
    if !is_valid_model_name(&req.name) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "model name must be 1..=50 chars of [A-Za-z0-9._-]",
        )
        .to_api_error());
    }
    let settings = Repos.voice.get_settings().await?;

    let spec = if let Some(url) = req.url.clone() {
        // Arbitrary URL (user-supplied) → SSRF-checked, unverified.
        model::ModelDownloadSpec {
            filename: model::model_filename(&req.name),
            name: req.name.clone(),
            url,
            expected_sha256: None,
            ssrf_check: true,
        }
    } else if let (Some(repo), Some(remote_filename)) = (req.repository.clone(), req.filename.clone())
    {
        // HF repo + file (user-supplied) → SSRF-checked, unverified. The remote
        // filename is used ONLY to build the fetch URL and MUST be a safe relative
        // path (no traversal); the ON-DISK filename is derived from the validated
        // `name`, never from the untrusted remote name (path-traversal guard).
        if !is_safe_remote_filename(&remote_filename) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "filename must be a safe relative path (no '..' or absolute path)",
            )
            .to_api_error());
        }
        let ext = if remote_filename.ends_with(".gguf") { "gguf" } else { "bin" };
        model::ModelDownloadSpec {
            url: model_catalog::hf_repo_url(&repo, &remote_filename),
            filename: format!("ggml-{}.{ext}", req.name),
            name: req.name.clone(),
            expected_sha256: None,
            ssrf_check: true,
        }
    } else {
        // Catalog: resolve against the configured (trusted) source + oid-verify.
        let (entries, reachable) = model_catalog::fetch_catalog(&settings.model_source_repo).await;
        if !reachable {
            return Err(AppError::bad_request(
                "VOICE_CATALOG_UNREACHABLE",
                "model source is unreachable; try again or upload the model file",
            )
            .to_api_error());
        }
        let entry = entries
            .into_iter()
            .find(|e| e.name == req.name)
            .ok_or_else(|| {
                AppError::not_found(&format!("catalog model {:?}", req.name)).to_api_error()
            })?;
        model::ModelDownloadSpec {
            url: model_catalog::download_url(&settings.model_source_repo, &entry.filename),
            filename: entry.filename,
            name: req.name.clone(),
            expected_sha256: entry.sha256,
            ssrf_check: false,
        }
    };

    let key = model_download_task::task_key(&spec.filename);
    let events_url = format!("/api/voice/models/downloads/{key}/events");
    let task = model_download_task::start_or_join(spec)
        .await
        .map_err(|e| e.to_api_error())?;
    Ok((
        StatusCode::OK,
        Json(DownloadModelStartedResponse {
            task_id: task.task_id,
            key: task.key.clone(),
            name: task.name.clone(),
            events_url,
        }),
    ))
}

pub fn download_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.downloadModel")
        .tag("Voice")
        .summary("Start a whisper model download (catalog / HF repo / URL)")
        .response::<200, Json<DownloadModelStartedResponse>>()
}

/// Cancel an in-flight model download by key.
pub async fn cancel_model_download(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    Path(key): Path<String>,
) -> ApiResult<impl IntoApiResponse> {
    let cancelled = model_download_task::cancel(&key).await;
    if cancelled {
        Ok((StatusCode::ACCEPTED, ()))
    } else {
        Err(AppError::not_found(&format!("active download {key:?}")).to_api_error())
    }
}

pub fn cancel_model_download_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.cancelModelDownload")
        .tag("Voice")
        .summary("Cancel an in-flight whisper model download")
        .response::<202, ()>()
}

/// List active (non-terminal) model downloads (page-reload repaint).
pub async fn list_active_model_downloads(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<Vec<SnapshotDto>>> {
    let tasks: Vec<Arc<ModelDownloadTask>> =
        MODEL_DOWNLOAD_TASKS.iter().map(|e| e.value().clone()).collect();
    let mut out = Vec::new();
    for t in tasks {
        let snap = snapshot_of(&t).await;
        if !matches!(snap.status.as_str(), "completed" | "failed") {
            out.push(snap);
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok((StatusCode::OK, Json(out)))
}

pub fn list_active_model_downloads_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.listModelDownloads")
        .tag("Voice")
        .summary("List active whisper model downloads")
        .response::<200, Json<Vec<SnapshotDto>>>()
}

/// Single-download poll snapshot (non-SSE fallback).
pub async fn get_model_download(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
    Path(key): Path<String>,
) -> ApiResult<Json<SnapshotDto>> {
    let task = model_download_task::get_task(&key)
        .ok_or_else(|| AppError::not_found(&format!("download {key:?}")))?;
    Ok((StatusCode::OK, Json(snapshot_of(&task).await)))
}

pub fn get_model_download_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.getModelDownload")
        .tag("Voice")
        .summary("Poll a single whisper model download snapshot")
        .response::<200, Json<SnapshotDto>>()
}

async fn snapshot_of(task: &Arc<ModelDownloadTask>) -> SnapshotDto {
    let g = task.state.lock().await;
    let percent = g
        .total_bytes
        .map(|t| if t == 0 { 0.0 } else { (g.bytes_received as f32 / t as f32) * 100.0 });
    SnapshotDto {
        task_id: task.task_id,
        key: task.key.clone(),
        name: task.name.clone(),
        status: format!("{:?}", g.status).to_lowercase(),
        bytes_received: g.bytes_received,
        total_bytes: g.total_bytes,
        percent,
        error: g.error.clone(),
    }
}

/// SSE stream of model-download events for a single task.
pub async fn subscribe_model_download_events(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
    Path(key): Path<String>,
) -> ApiResult<
    axum::response::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, axum::Error>>>,
> {
    use axum::response::sse::{Event, KeepAlive, Sse};

    let task = model_download_task::get_task(&key)
        .ok_or_else(|| AppError::not_found(&format!("download task {key:?}")).to_api_error())?;

    // Subscribe BEFORE snapshotting so no live event is dropped in the gap.
    let mut rx = task.events.subscribe();
    let (initial_status, replay, terminal_complete, terminal_err) = {
        let g = task.state.lock().await;
        (g.status, g.progress.clone(), g.complete.clone(), g.error.clone())
    };
    let task_clone = task.clone();
    let stream = async_stream::stream! {
        yield Ok::<Event, axum::Error>(SSEModelDownloadEvent::Connected(SSEModelDownloadConnectedData {
            task_id: task_clone.task_id,
            key: task_clone.key.clone(),
            name: task_clone.name.clone(),
            status: initial_status,
        }).into());
        for p in replay {
            yield Ok(SSEModelDownloadEvent::Progress(p).into());
        }
        // Terminal-already: replay the terminal event + CLOSE (never enter rx.recv,
        // whose Complete/Failed frame already fired before this subscription).
        if let Some(c) = terminal_complete {
            yield Ok(SSEModelDownloadEvent::Complete(c).into());
            return;
        }
        if let Some(e) = terminal_err {
            yield Ok(SSEModelDownloadEvent::Failed(super::model_download_task::SSEModelDownloadFailedData { error: e }).into());
            return;
        }
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let is_terminal = matches!(ev, SSEModelDownloadEvent::Complete(_) | SSEModelDownloadEvent::Failed(_));
                    yield Ok(ev.into());
                    if is_terminal { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    };
    Ok((StatusCode::OK, Sse::new(stream).keep_alive(KeepAlive::default())))
}

pub fn subscribe_model_download_events_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.subscribeModelDownloadEvents")
        .tag("Voice")
        .summary("SSE stream of whisper model download progress")
}

// ─────────────────────────────── upload ──────────────────────────────────

/// Upload a whisper model file (multipart). Validates the whisper magic + size
/// cap, stores under `voice-models/`, and registers an unverified row.
pub async fn upload_model(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    origin: SyncOrigin,
    mut multipart: Multipart,
) -> ApiResult<Json<VoiceModel>> {
    let mut name: Option<String> = None;
    let mut upload: Option<model::UploadTemp> = None;
    let mut orig_filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request("UPLOAD_ERROR", format!("multipart error: {e}")).to_api_error())?
    {
        match field.name() {
            Some("name") => {
                name = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::bad_request("UPLOAD_ERROR", format!("read name: {e}")).to_api_error())?,
                );
            }
            Some("file") => {
                orig_filename = field.file_name().map(|s| s.to_string());
                // Stream to a temp file (cap enforced as bytes arrive — never
                // buffers the whole multi-GB file in RAM).
                match model::stream_upload_to_temp(field).await {
                    Ok(u) => upload = Some(u),
                    Err(e) => return Err(e.to_api_error()),
                }
            }
            _ => {}
        }
    }

    // Validate name + magic; discard the temp on any rejection.
    let upload = upload.ok_or_else(|| {
        AppError::bad_request("VALIDATION_ERROR", "missing file").to_api_error()
    })?;
    let reject = |u: &model::UploadTemp, code: &'static str, msg: &'static str| {
        model::discard_temp(&u.tmp);
        AppError::bad_request(code, msg).to_api_error()
    };
    let name = match name.filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => return Err(reject(&upload, "VALIDATION_ERROR", "missing model name")),
    };
    if !is_valid_model_name(&name) {
        return Err(reject(
            &upload,
            "VALIDATION_ERROR",
            "model name must be 1..=50 chars of [A-Za-z0-9._-]",
        ));
    }
    if !model::has_whisper_magic(&upload.head) {
        return Err(reject(
            &upload,
            "VOICE_MODEL_INVALID",
            "file is not a whisper ggml/GGUF model (bad magic)",
        ));
    }

    // Filename: keep a .gguf upload's extension, else the ggml-<name>.bin form.
    let filename = match orig_filename.as_deref() {
        Some(f) if f.ends_with(".gguf") => format!("ggml-{name}.gguf"),
        _ => model::model_filename(&name),
    };
    model::finalize_upload_temp(&upload.tmp, &filename).map_err(|e| e.to_api_error())?;
    let (size_bytes, sha256) = (upload.size, upload.sha256.clone());
    let row = Repos
        .voice_model
        .upsert(
            &name,
            &filename,
            VoiceModelSource::Upload,
            None,
            size_bytes as i64,
            Some(&sha256),
            false,
        )
        .await
        .map_err(|e| e.to_api_error())?;

    sync_publish(
        SyncEntity::VoiceModel,
        SyncAction::Create,
        row.id,
        Audience::perm::<VoiceAdminRead>(),
        origin.0,
    );
    let settings = Repos.voice.get_settings().await?;
    Ok((StatusCode::OK, Json(to_api(row, &settings.model, None))))
}

pub fn upload_model_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.uploadModel")
        .tag("Voice")
        .summary("Upload a whisper model file")
        .response::<200, Json<VoiceModel>>()
}
