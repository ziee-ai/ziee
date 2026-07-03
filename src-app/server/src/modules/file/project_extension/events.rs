// Events for the project↔file relationship.
//
// Relocated from `modules/project/events.rs` as part of the project↔file
// inversion. Project module's `ProjectEvent` no longer carries
// `FileAttached`/`FileDetached` variants — those are now under the
// `AppEvent::FileProject(FileProjectEvent)` arm.
//
// Constructor helpers (`attached`, `detached`) build `AppEvent` directly
// so file handlers can call `event_bus.emit_async(FileProjectEvent::attached(...))`
// without importing the event-bus or AppEvent types.

use uuid::Uuid;

// Constructed + emitted by the file handlers (attached/detached below), but the
// payload fields are not yet read by any subscriber — kept as the event's
// forward-compatible shape. Narrow allow replaces the old module blanket.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FileProjectEvent {
    /// A file has been attached to a project. Emitted by the
    /// `POST /api/projects/{id}/files` and
    /// `POST /api/projects/{id}/files/upload` handlers — both are
    /// owned by the file module's project-extension.
    Attached {
        project_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
    },
    /// A file has been detached from a project. Emitted by the
    /// `DELETE /api/projects/{id}/files/{file_id}` handler.
    Detached {
        project_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
    },
}

impl FileProjectEvent {
    pub fn attached(project_id: Uuid, file_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::FileProject(FileProjectEvent::Attached {
            project_id,
            file_id,
            user_id,
        })
    }

    pub fn detached(project_id: Uuid, file_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::FileProject(FileProjectEvent::Detached {
            project_id,
            file_id,
            user_id,
        })
    }
}
