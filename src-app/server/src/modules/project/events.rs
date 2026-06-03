// Project module events
// Currently emitted-only (no in-module handlers) — other modules can
// subscribe in future to react to project lifecycle changes.
#![allow(dead_code)]

use uuid::Uuid;

// `FileAttached`/`FileDetached` variants previously lived here.
// They moved to `crate::modules::file::project_extension::events::FileProjectEvent`
// as part of the project↔file inversion — the file module now owns the
// `project_files` join table and its lifecycle events. See the
// `AppEvent::FileProject(...)` arm in `core/events.rs`.

#[derive(Debug, Clone)]
pub enum ProjectEvent {
    Created { project_id: Uuid, user_id: Uuid },
    Updated { project_id: Uuid, user_id: Uuid },
    Deleted { project_id: Uuid, user_id: Uuid },
    /// A conversation has been attached to (or re-attached across) a
    /// project. Emitted by `POST /projects/{id}/conversations/{conv_id}`.
    /// The MCP snapshot has already been refreshed when this fires.
    ConversationAttached {
        conversation_id: Uuid,
        project_id: Uuid,
        from_project_id: Option<Uuid>,
        user_id: Uuid,
    },
    /// A conversation has been detached from its project. Emitted by
    /// `DELETE /projects/{id}/conversations/{conv_id}`. The MCP
    /// snapshot row has already been cleared.
    ConversationDetached {
        conversation_id: Uuid,
        project_id: Uuid,
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
    pub fn conversation_attached(
        conversation_id: Uuid,
        project_id: Uuid,
        from_project_id: Option<Uuid>,
        user_id: Uuid,
    ) -> crate::core::AppEvent {
        crate::core::AppEvent::Project(ProjectEvent::ConversationAttached {
            conversation_id,
            project_id,
            from_project_id,
            user_id,
        })
    }
    pub fn conversation_detached(
        conversation_id: Uuid,
        project_id: Uuid,
        user_id: Uuid,
    ) -> crate::core::AppEvent {
        crate::core::AppEvent::Project(ProjectEvent::ConversationDetached {
            conversation_id,
            project_id,
            user_id,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Constructor-shape tests for the conversation-attached/detached
    //! events. These prove the public emit surface that
    //! `attach_conversation` / `detach_conversation` handlers depend on:
    //! a renamed variant or a constructor-field omission would surface
    //! here before it broke a real subscriber in the wild.
    //!
    //! Wire-up coverage (HTTP handler actually calling emit_async)
    //! requires an in-process EventBus + a recorder handler. The
    //! integration TestServer is a separate process, so cross-process
    //! observation of the in-memory bus isn't possible today; that
    //! gap closes naturally the moment any module subscribes to these
    //! events with an observable side effect (the existing handler-
    //! triggered DB writes already cover that pathway end-to-end).
    use super::*;
    use crate::core::AppEvent;
    use uuid::Uuid;

    #[test]
    fn conversation_attached_constructor_yields_expected_variant() {
        let conv = Uuid::new_v4();
        let project = Uuid::new_v4();
        let from = Some(Uuid::new_v4());
        let user = Uuid::new_v4();

        let evt = ProjectEvent::conversation_attached(conv, project, from, user);
        match evt {
            AppEvent::Project(ProjectEvent::ConversationAttached {
                conversation_id,
                project_id,
                from_project_id,
                user_id,
            }) => {
                assert_eq!(conversation_id, conv);
                assert_eq!(project_id, project);
                assert_eq!(from_project_id, from);
                assert_eq!(user_id, user);
            }
            other => panic!("expected ConversationAttached, got {:?}", other),
        }
    }

    #[test]
    fn conversation_attached_constructor_supports_none_from_project() {
        // Initial attach (no prior project) — from_project_id is None.
        let evt = ProjectEvent::conversation_attached(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            Uuid::new_v4(),
        );
        match evt {
            AppEvent::Project(ProjectEvent::ConversationAttached {
                from_project_id, ..
            }) => assert!(from_project_id.is_none()),
            other => panic!("expected ConversationAttached, got {:?}", other),
        }
    }

    #[test]
    fn conversation_detached_constructor_yields_expected_variant() {
        let conv = Uuid::new_v4();
        let project = Uuid::new_v4();
        let user = Uuid::new_v4();

        let evt = ProjectEvent::conversation_detached(conv, project, user);
        match evt {
            AppEvent::Project(ProjectEvent::ConversationDetached {
                conversation_id,
                project_id,
                user_id,
            }) => {
                assert_eq!(conversation_id, conv);
                assert_eq!(project_id, project);
                assert_eq!(user_id, user);
            }
            other => panic!("expected ConversationDetached, got {:?}", other),
        }
    }
}
