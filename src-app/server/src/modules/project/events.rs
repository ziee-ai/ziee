// Project module events
// Currently emitted-only (no in-module handlers) — other modules can
// subscribe in future to react to project lifecycle changes.
#![allow(dead_code)]

use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum ProjectEvent {
    Created { project_id: Uuid, user_id: Uuid },
    Updated { project_id: Uuid, user_id: Uuid },
    Deleted { project_id: Uuid, user_id: Uuid },
    FileAttached {
        project_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
    },
    FileDetached {
        project_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
    },
    ConversationMoved {
        conversation_id: Uuid,
        from_project_id: Option<Uuid>,
        to_project_id: Option<Uuid>,
        user_id: Uuid,
    },
}

impl ProjectEvent {
    pub fn created(project_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::Project(ProjectEvent::Created {
            project_id,
            user_id,
        })
    }
    pub fn updated(project_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::Project(ProjectEvent::Updated {
            project_id,
            user_id,
        })
    }
    pub fn deleted(project_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::Project(ProjectEvent::Deleted {
            project_id,
            user_id,
        })
    }
    pub fn file_attached(project_id: Uuid, file_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::Project(ProjectEvent::FileAttached {
            project_id,
            file_id,
            user_id,
        })
    }
    pub fn file_detached(project_id: Uuid, file_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::Project(ProjectEvent::FileDetached {
            project_id,
            file_id,
            user_id,
        })
    }
    pub fn conversation_moved(
        conversation_id: Uuid,
        from_project_id: Option<Uuid>,
        to_project_id: Option<Uuid>,
        user_id: Uuid,
    ) -> crate::core::AppEvent {
        crate::core::AppEvent::Project(ProjectEvent::ConversationMoved {
            conversation_id,
            from_project_id,
            to_project_id,
            user_id,
        })
    }
}
