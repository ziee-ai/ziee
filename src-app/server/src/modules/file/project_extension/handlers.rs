// HTTP handlers for the project↔file routes mounted at
// `/api/projects/{id}/files*`. Relocated from `modules/project/handlers.rs`
// as part of the project↔file inversion — the project module no longer
// owns this code.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Extension, Multipart, Path},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::{EventBus, Repos};
use crate::modules::file::handlers::upload::upload_file_inner;
use crate::modules::file::models::File as FileEntity;
use crate::modules::file::permissions::FilesUpload;
use crate::modules::file::project_extension::events::FileProjectEvent;
use crate::modules::file::project_extension::models::{
    AttachFileRequest, ProjectFileListResponse,
};
use crate::modules::file::project_extension::repository::PROJECT_MAX_FILES;
use crate::modules::permissions::{extractors::RequirePermissions, with_permission};
use crate::modules::project::permissions::{ProjectsEdit, ProjectsRead};
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

#[debug_handler]
pub async fn list_project_files(
    auth: RequirePermissions<(ProjectsRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ProjectFileListResponse>> {
    let _ = Repos
        .project
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;
    let response = Repos.project_files.list_files(id).await?;
    Ok((StatusCode::OK, Json(response)))
}

pub fn list_project_files_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsRead,)>(op)
        .id("Project.listFiles")
        .tag("Projects")
        .summary("List files attached to a project")
        .response::<200, Json<ProjectFileListResponse>>()
        .response_with::<404, (), _>(|res| res.description("Project not found"))
}

#[debug_handler]
pub async fn attach_file(
    auth: RequirePermissions<(ProjectsEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<AttachFileRequest>,
) -> ApiResult<()> {
    // Project must exist and be owned by the user.
    let project = Repos
        .project
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;

    // File must exist and be owned by the same user. Both checks are
    // load-bearing — without them, user B could attach A's file (file
    // pull) or attach to A's project (project pollution).
    //
    // 404 (not 403) on the cross-tenant case so we don't leak the
    // existence of foreign files (audit N2). The handler-side test
    // `cannot_attach_other_users_file` accepts EITHER 403 or 404, so
    // tightening to 404 doesn't regress.
    let file = Repos
        .file
        .get_by_id(request.file_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    if file.user_id != auth.user.id {
        return Err(AppError::not_found("File").into());
    }

    // Race-free attach: takes a row lock on the project, recounts
    // under the lock, rejects with 422 if at cap, INSERTs in the same
    // transaction. Closes audit B1 (concurrent attaches at count=99
    // could both pass a pre-check and exceed the cap).
    let newly_attached = Repos
        .project_files
        .attach_file_capped(project.id, file.id, PROJECT_MAX_FILES)
        .await?;
    if newly_attached {
        event_bus.emit_async(FileProjectEvent::attached(
            project.id,
            file.id,
            auth.user.id,
        ));
        // The project's file set changed → refresh the owner's other devices.
        sync_publish(
            SyncEntity::Project,
            SyncAction::Update,
            project.id,
            Audience::owner(auth.user.id),
            origin.0,
        );
    }
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn attach_file_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsEdit,)>(op)
        .id("Project.attachFile")
        .tag("Projects")
        .summary("Attach a file to a project")
        .description(
            "Attach an existing file (by ID) to this project. Idempotent (re-attaching the same \
             file is a no-op). The file must be owned by the same user as the project.\n\
             \n\
             Error codes:\n\
             - `PROJECT_FILE_COUNT_CAP` (422) — project already has the max files (100).",
        )
        .response_with::<204, (), _>(|res| res.description("File attached"))
        .response_with::<404, (), _>(|res| {
            res.description("Project or file not found (or file belongs to another user)")
        })
        .response_with::<422, (), _>(|res| res.description("File count cap reached"))
}

/// Combined upload+attach in one round-trip. Uploads via the shared
/// `upload_file_inner` from the file module (so size/MIME/quota/zipbomb
/// validation matches the standalone POST /files exactly), then attaches
/// the new file to the project. Best-effort transactional: if the attach
/// step fails after the file has been created, the file remains in the
/// user's library (they can attach manually via POST /projects/{id}/files).
#[debug_handler]
pub async fn upload_and_attach_file(
    auth: RequirePermissions<(ProjectsEdit, FilesUpload)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
    multipart: Multipart,
) -> ApiResult<Json<FileEntity>> {
    // 1. Verify project ownership.
    let project = Repos
        .project
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;

    // 2. Pre-flight file count cap (advisory only — attach_file_capped
    //    recounts under a row lock and is the authoritative gate).
    let count = Repos.project_files.count_files(project.id).await?;
    if count >= PROJECT_MAX_FILES {
        return Err(AppError::unprocessable_entity(
            "PROJECT_FILE_COUNT_CAP",
            format!("Project file count cap ({}) reached", PROJECT_MAX_FILES),
        )
        .into());
    }

    // 3. Upload via the shared core (validates size, MIME, quota,
    //    zip-bombs; creates the files row + storage entries).
    let file = upload_file_inner(auth.user.id, multipart, origin.0).await?;

    // 4. Arm a Drop guard that fires if we exit this function before
    //    disarming it — covers BOTH the attach-failure case (B2) AND
    //    the cancelled-future case (N5: client disconnects between
    //    upload-complete and attach-success).
    let mut cleanup = OrphanFileCleanup::new(file.id, auth.user.id);

    // 5. Race-free attach (B1).
    let newly_attached = Repos
        .project_files
        .attach_file_capped(project.id, file.id, PROJECT_MAX_FILES)
        .await?;

    // Attach succeeded — disarm the guard so we keep the file.
    cleanup.disarm();

    if newly_attached {
        event_bus.emit_async(FileProjectEvent::attached(
            project.id,
            file.id,
            auth.user.id,
        ));
        sync_publish(
            SyncEntity::Project,
            SyncAction::Update,
            project.id,
            Audience::owner(auth.user.id),
            origin.0,
        );
    }

    Ok((StatusCode::CREATED, Json(file)))
}

/// RAII guard that deletes a freshly-uploaded file row + storage
/// artifacts when dropped, UNLESS `disarm()` was called first.
/// Used by `upload_and_attach_file` (audit B2 + N5) so an attach
/// failure OR a cancelled-future cleanup happens reliably without
/// requiring an explicit `if let Err` arm.
struct OrphanFileCleanup {
    file_id: Uuid,
    user_id: Uuid,
    armed: bool,
}

impl OrphanFileCleanup {
    fn new(file_id: Uuid, user_id: Uuid) -> Self {
        Self {
            file_id,
            user_id,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for OrphanFileCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => {
                tracing::warn!(
                    file_id = %self.file_id,
                    user_id = %self.user_id,
                    "OrphanFileCleanup: no Tokio runtime available; skipping cleanup"
                );
                return;
            }
        };
        let file_id = self.file_id;
        let user_id = self.user_id;
        // block_in_place is required: Handle::block_on panics when called
        // from an async context on a multi-thread runtime.  block_in_place
        // yields the current worker thread so block_on can safely re-enter
        // the runtime on this thread.  Mirrors the pattern in
        // core/database/mod.rs::run_cleanup_blocking.
        tokio::task::block_in_place(move || {
            handle.block_on(async move {
            tracing::info!(
                %file_id, %user_id,
                "OrphanFileCleanup: deleting orphaned file (attach failed or future cancelled)"
            );
            let row_ok = match Repos.file.delete(file_id, user_id).await {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!(
                        %file_id, %user_id, error = ?e,
                        "OrphanFileCleanup: failed to delete file row"
                    );
                    false
                }
            };
            let storage_ok = match crate::modules::file::storage::manager::get_file_storage()
                .delete_all(user_id, file_id)
                .await
            {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!(
                        %file_id, %user_id, error = ?e,
                        "OrphanFileCleanup: failed to delete storage artifacts"
                    );
                    false
                }
            };
            if row_ok && storage_ok {
                tracing::info!(
                    %file_id, %user_id,
                    "OrphanFileCleanup: orphaned file deleted successfully"
                );
            }
        });
        });
    }
}

/// OpenAPI description for the upload-and-attach endpoint. Extracted to a const
/// so the size-cap copy stays cap-agnostic (the per-file cap is configurable via
/// `config.server.max_file_upload_mb`) and a unit test can guard against the
/// stale hardcoded-limit copy reappearing.
const UPLOAD_AND_ATTACH_DESCRIPTION: &str =
    "**Multipart/form-data** upload. Send the file bytes in a part named `file` with a \
     filename (Content-Disposition: form-data; name=\"file\"; filename=\"<name>\"). The \
     server creates the file row + storage artifacts AND attaches the new file to the \
     project in one round-trip. Failures roll back the upload via a Drop-guard so no \
     orphans survive client disconnects.\n\
     \n\
     Enforces the file module's configurable per-file size cap, per-user quota (10 GiB), \
     MIME sniffing + smuggling rejection, and the project's 100-file cap.\n\
     \n\
     Error codes:\n\
     - `MISSING_FILE` (400) — no `file` part in the multipart body.\n\
     - `FILE_TOO_LARGE` (400) — file exceeds the configured per-file size cap.\n\
     - `STORAGE_QUOTA_EXCEEDED` (400) — per-user quota exhausted.\n\
     - `MIME_MISMATCH` (400) — declared MIME doesn't match sniffed bytes.\n\
     - `ZIP_BOMB_DETECTED` (400) — OOXML/ODF container expansion exceeds limits.\n\
     - `PROJECT_FILE_COUNT_CAP` (422) — project already has 100 files.";

pub fn upload_and_attach_file_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsEdit, FilesUpload)>(op)
        .id("Project.uploadAndAttachFile")
        .tag("Projects")
        .summary("Upload a file and attach it to a project (multipart)")
        .description(UPLOAD_AND_ATTACH_DESCRIPTION)
        .response::<201, Json<FileEntity>>()
        .response_with::<400, (), _>(|res| res.description("Upload-validation error"))
        .response_with::<404, (), _>(|res| res.description("Project not found"))
        .response_with::<422, (), _>(|res| res.description("Project file count cap reached"))
}

#[debug_handler]
pub async fn detach_file(
    auth: RequirePermissions<(ProjectsEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path((id, file_id)): Path<(Uuid, Uuid)>,
    origin: SyncOrigin,
) -> ApiResult<()> {
    let project = Repos
        .project
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;

    let removed = Repos.project_files.detach_file(project.id, file_id).await?;
    if !removed {
        return Err(AppError::not_found("Project file").into());
    }
    event_bus.emit_async(FileProjectEvent::detached(
        project.id,
        file_id,
        auth.user.id,
    ));
    sync_publish(
        SyncEntity::Project,
        SyncAction::Update,
        project.id,
        Audience::owner(auth.user.id),
        origin.0,
    );
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn detach_file_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsEdit,)>(op)
        .id("Project.detachFile")
        .tag("Projects")
        .summary("Detach a file from a project")
        .description(
            "Remove the project↔file membership. Does NOT delete the underlying file (it may be \
             attached to other projects or used per-message in conversations).",
        )
        .response_with::<204, (), _>(|res| res.description("File detached"))
        .response_with::<404, (), _>(|res| res.description("Project or file not attached"))
}

#[cfg(test)]
mod description_tests {
    use super::UPLOAD_AND_ATTACH_DESCRIPTION;

    #[test]
    fn upload_description_has_no_stale_hardcoded_cap() {
        // The per-file cap is configurable; the docs must not cite a fixed size.
        assert!(
            !UPLOAD_AND_ATTACH_DESCRIPTION.contains("100 MiB")
                && !UPLOAD_AND_ATTACH_DESCRIPTION.contains("100 MB"),
            "upload-and-attach docs must not hardcode a size cap"
        );
        assert!(
            UPLOAD_AND_ATTACH_DESCRIPTION.contains("configurable per-file size cap"),
            "upload-and-attach docs must describe the configurable per-file size cap"
        );
    }
}
