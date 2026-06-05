//! Sync event wire types + the single, auditable audience routing table.
//!
//! Events are **notify-and-refetch**: the wire payload is only
//! `{entity, action, id}` — never row data. The client refetches the
//! changed entity via its existing permission-checked REST endpoint, so
//! the SSE channel never carries anything sensitive.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// The kind of entity that changed. Serialized snake_case to match the
/// frontend's `sync:<entity>` event vocabulary.
///
/// ADD a variant here when wiring a new domain: `audience_kind`'s match
/// is exhaustive, so the build fails until the new entity is assigned an
/// audience — a new syncable entity can never silently default to a
/// broadcast (the dangerous default becomes a compile error, not a leak).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncEntity {
    // --- Owner-scoped (delivered only to the owning user) ---
    Project,
    /// A user-created assistant (NOT a system template — that's a separate
    /// entity with a different audience).
    Assistant,
    /// A user-owned MCP server (NOT a system/group-shared server).
    McpServer,
    Memory,
    MemorySettings,
    /// A user's own profile (display fields / active state). Emitted to the
    /// affected user — e.g. when an admin edits their account — so their
    /// other devices re-bootstrap `/auth/me`.
    Profile,
    /// A user's saved LLM-provider API key (`id` is the provider id; only
    /// masked state is ever exposed, and only via refetch).
    ApiKey,

    // --- Admin-permission-scoped (delivered to holders of the read perm) ---
    /// Admin view of an LLM provider (full admin provider table).
    LlmProvider,
    /// Admin view of an LLM model.
    LlmModel,
    /// A user group (admin tables; includes group permissions).
    Group,
    /// A user account (admin users list).
    User,
    /// A shared assistant template (visible to any user who can read
    /// templates — non-secret, uniform view).
    AssistantTemplate,
    /// Admin view of a system (deployment-shared) MCP server.
    McpServerSystem,
    /// An LLM repository (admin).
    LlmRepository,
    /// A local-runtime engine version (admin).
    RuntimeVersion,
    /// Deployment-wide local-runtime engine settings (singleton).
    RuntimeSettings,
    /// Deployment-wide memory admin settings (singleton).
    MemoryAdminSettings,
    /// Code-sandbox resource-limit settings (singleton).
    CodeSandboxSettings,
    /// Hub catalog settings (singleton).
    HubSettings,

    // --- Group-scoped user view (delivered to holders of the user read
    // perm; safe because we only NOTIFY — each recipient refetches its own
    // group-scoped, sanitized view; the only disclosure is "something
    // changed"). Emitted ALONGSIDE the admin entity above on the same
    // mutation, so admins and regular users each refresh their own surface. ---
    /// A user's accessible-providers (with enabled models) view changed.
    UserLlmProvider,
    /// A user's accessible (system) MCP-servers view changed.
    UserMcpServer,

    // --- Owner-scoped signal ---
    /// The user's session/permissions changed (group membership or a group's
    /// permissions were edited) — the client re-bootstraps `/auth/me`. `id`
    /// is the affected user id.
    Session,
}

/// What happened to the entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    Create,
    Update,
    Delete,
}

/// The change notification pushed to clients. Notify-and-refetch only.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SyncEvent {
    pub entity: SyncEntity,
    pub action: SyncAction,
    pub id: Uuid,
}

/// Handshake payload: the server-assigned connection id. The client
/// echoes it back via the `X-Sync-Connection-Id` header on mutations so
/// the fan-out can skip the originating connection (self-echo suppression).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SyncConnectedData {
    pub connection_id: Uuid,
}

crate::sse_event_enum! {
    #[derive(Debug, Clone, Serialize, JsonSchema)]
    pub enum SyncSseEvent {
        Connected(SyncConnectedData),
        Sync(SyncEvent),
    }
}

/// Resolved delivery scope for one event. Built from `audience_kind` plus
/// the owner id the emitting handler supplies for owner-scoped entities.
#[derive(Debug, Clone)]
pub enum Audience {
    /// Only the owning user's connections.
    Owner(Uuid),
    /// Only connections whose permission snapshot satisfies this perm
    /// (admins always qualify).
    Permission(&'static str),
    /// Every authenticated connection.
    Everyone,
}

/// The central authorization table: maps each entity to its delivery
/// scope. This is the ONE place an audience is decided — keep it
/// exhaustive and review it for leaks.
#[derive(Debug, Clone, Copy)]
enum AudienceKind {
    Owner,
    Permission(&'static str),
    Everyone,
}

fn audience_kind(entity: SyncEntity) -> AudienceKind {
    match entity {
        SyncEntity::Project
        | SyncEntity::Assistant
        | SyncEntity::McpServer
        | SyncEntity::Memory
        | SyncEntity::MemorySettings
        | SyncEntity::Profile
        | SyncEntity::ApiKey => AudienceKind::Owner,

        SyncEntity::LlmProvider => AudienceKind::Permission("llm_providers::read"),
        SyncEntity::LlmModel => AudienceKind::Permission("llm_models::read"),
        SyncEntity::UserLlmProvider => {
            AudienceKind::Permission("user_llm_providers::read")
        }
        SyncEntity::Group => AudienceKind::Permission("groups::read"),
        SyncEntity::User => AudienceKind::Permission("users::read"),
        // Templates are uniform + non-secret — notify every authenticated
        // connection (matches the plan). The refetch is still authorized, so a
        // user who can't read templates just no-ops on the 403.
        SyncEntity::AssistantTemplate => AudienceKind::Everyone,
        SyncEntity::McpServerSystem => {
            AudienceKind::Permission("mcp_servers_admin::read")
        }
        SyncEntity::LlmRepository => {
            AudienceKind::Permission("llm_repositories::read")
        }
        // The version catalogue is gated by its OWN read perm (split from
        // `llm_local_runtime::read` instance-telemetry on purpose) — match the
        // permission the client's refetch (`RuntimeVersion.list`) requires.
        SyncEntity::RuntimeVersion => {
            AudienceKind::Permission("llm_local_runtime::versions_read")
        }
        SyncEntity::RuntimeSettings => {
            AudienceKind::Permission("llm_local_runtime::settings_read")
        }
        SyncEntity::MemoryAdminSettings => {
            AudienceKind::Permission("memory::admin::read")
        }
        SyncEntity::CodeSandboxSettings => {
            AudienceKind::Permission("code_sandbox::resource_limits::read")
        }
        SyncEntity::HubSettings => AudienceKind::Permission("hub::catalog::read"),
        SyncEntity::UserMcpServer => AudienceKind::Permission("mcp_servers::read"),

        SyncEntity::Session => AudienceKind::Owner,
    }
}

/// Publish a change notification to the appropriate audience.
///
/// `owner` is required for owner-scoped entities (the central table
/// decides which those are; a missing owner there is a bug and the event
/// is dropped rather than mis-delivered). `origin_conn` is the
/// originating SSE connection, skipped to suppress self-echo.
pub fn publish(
    entity: SyncEntity,
    action: SyncAction,
    id: Uuid,
    owner: Option<Uuid>,
    origin_conn: Option<Uuid>,
) {
    let audience = match audience_kind(entity) {
        AudienceKind::Owner => match owner {
            Some(u) => Audience::Owner(u),
            None => {
                tracing::error!(
                    ?entity,
                    "owner-scoped sync event emitted without an owner id; dropping"
                );
                return;
            }
        },
        AudienceKind::Permission(p) => Audience::Permission(p),
        AudienceKind::Everyone => Audience::Everyone,
    };

    super::registry::registry().deliver(audience, SyncEvent { entity, action, id }, origin_conn);
}

/// Fan a `Session` permissions-changed signal out to many users at once
/// (used by group-permission edits that affect every member). Delivers via a
/// SINGLE registry lock acquisition rather than one `publish` call per user.
pub fn publish_session_to_users(user_ids: &[Uuid], origin_conn: Option<Uuid>) {
    super::registry::registry().deliver_session_to_users(user_ids, origin_conn);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_scoped_entities_route_to_owner() {
        for e in [
            SyncEntity::Project,
            SyncEntity::Assistant,
            SyncEntity::McpServer,
            SyncEntity::Memory,
            SyncEntity::MemorySettings,
            SyncEntity::Profile,
            SyncEntity::ApiKey,
            SyncEntity::Session,
        ] {
            assert!(
                matches!(audience_kind(e), AudienceKind::Owner),
                "{e:?} should be Owner-scoped"
            );
        }
    }

    #[test]
    fn admin_entities_route_to_their_read_permission() {
        let cases = [
            (SyncEntity::LlmProvider, "llm_providers::read"),
            (SyncEntity::LlmModel, "llm_models::read"),
            (SyncEntity::Group, "groups::read"),
            (SyncEntity::User, "users::read"),
            (SyncEntity::McpServerSystem, "mcp_servers_admin::read"),
            (SyncEntity::LlmRepository, "llm_repositories::read"),
            (SyncEntity::RuntimeVersion, "llm_local_runtime::versions_read"),
            (SyncEntity::RuntimeSettings, "llm_local_runtime::settings_read"),
            (SyncEntity::MemoryAdminSettings, "memory::admin::read"),
            (
                SyncEntity::CodeSandboxSettings,
                "code_sandbox::resource_limits::read",
            ),
            (SyncEntity::HubSettings, "hub::catalog::read"),
        ];
        for (e, perm) in cases {
            match audience_kind(e) {
                AudienceKind::Permission(p) => assert_eq!(p, perm, "{e:?}"),
                other => panic!("{e:?} expected Permission, got {other:?}"),
            }
        }
    }

    #[test]
    fn assistant_templates_route_to_everyone() {
        assert!(matches!(
            audience_kind(SyncEntity::AssistantTemplate),
            AudienceKind::Everyone
        ));
    }

    #[test]
    fn user_facing_views_route_to_the_user_read_permission() {
        // Group-scoped visibility — safe because notify-only: each recipient
        // refetches its OWN scoped view.
        match audience_kind(SyncEntity::UserLlmProvider) {
            AudienceKind::Permission(p) => assert_eq!(p, "user_llm_providers::read"),
            other => panic!("expected Permission, got {other:?}"),
        }
        match audience_kind(SyncEntity::UserMcpServer) {
            AudienceKind::Permission(p) => assert_eq!(p, "mcp_servers::read"),
            other => panic!("expected Permission, got {other:?}"),
        }
    }

    #[test]
    fn wire_payload_is_notify_only_snake_case() {
        let e = SyncEvent {
            entity: SyncEntity::McpServerSystem,
            action: SyncAction::Update,
            id: Uuid::nil(),
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"entity\":\"mcp_server_system\""), "{json}");
        assert!(json.contains("\"action\":\"update\""), "{json}");
        // Notify-and-refetch: the wire carries ONLY entity/action/id — never
        // row data. Guard against accidentally widening the payload.
        let obj: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&json).unwrap();
        assert_eq!(obj.len(), 3, "only entity/action/id may cross the wire: {json}");
    }

    #[test]
    fn entity_names_match_the_frontend_sync_vocabulary() {
        let cases = [
            (SyncEntity::Project, "project"),
            (SyncEntity::UserLlmProvider, "user_llm_provider"),
            (SyncEntity::MemorySettings, "memory_settings"),
            (SyncEntity::Session, "session"),
        ];
        for (e, name) in cases {
            assert_eq!(serde_json::to_string(&e).unwrap(), format!("\"{name}\""));
        }
    }
}
