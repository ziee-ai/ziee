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

    // --- Group-scoped user view (delivered to holders of the user read
    // perm; safe because we only NOTIFY — each recipient refetches its own
    // group-scoped, sanitized view; the only disclosure is "something
    // changed"). Emitted ALONGSIDE the admin entity above on the same
    // mutation, so admins and regular users each refresh their own surface. ---
    /// A user's accessible-providers (with enabled models) view changed.
    UserLlmProvider,
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
        | SyncEntity::ApiKey => AudienceKind::Owner,

        SyncEntity::LlmProvider => AudienceKind::Permission("llm_providers::read"),
        SyncEntity::LlmModel => AudienceKind::Permission("llm_models::read"),
        SyncEntity::UserLlmProvider => {
            AudienceKind::Permission("user_llm_providers::read")
        }
        SyncEntity::Group => AudienceKind::Permission("groups::read"),
        SyncEntity::User => AudienceKind::Permission("users::read"),
        SyncEntity::AssistantTemplate => {
            AudienceKind::Permission("assistant_templates::read")
        }
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
