//! Sync event wire types + the single, auditable audience routing table.
//!
//! Events are **notify-and-refetch**: the wire payload is only
//! `{entity, action, id}` — never row data. The client refetches the
//! changed entity via its existing permission-checked REST endpoint, so
//! the SSE channel never carries anything sensitive.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::modules::permissions::{PermissionCheck, PermissionList};

/// The kind of entity that changed. Serialized snake_case to match the
/// frontend's `sync:<entity>` event vocabulary.
///
/// ADD a variant here when wiring a new domain. NOTE: there is no central
/// `audience_kind` match — each emitting handler picks the `Audience`
/// explicitly at the `publish` call site (`Audience::owner(..)` /
/// `Audience::perm::<P>()` / `Audience::everyone()`). So adding a variant
/// does NOT force an audience assignment at compile time; the author must
/// choose the correct audience at every emit site for the new entity (an
/// owner-scoped entity broadcast to everyone would be a leak). Keep new
/// entities' audiences aligned with the read-permission gating their
/// refetch endpoint enforces.
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
    /// A chat conversation owned by the user. `id` is the conversation id.
    /// Emitted on create/rename/delete, on each completed message turn, and on
    /// branch/message edits — the recipient refetches the list and (if open)
    /// the conversation's messages. Live assistant TOKENS do NOT ride this
    /// stream; they go over the dedicated `chat/stream` token channel.
    Conversation,
    /// A user-owned file whose version set / head changed — via a REST restore,
    /// the built-in `files_mcp` write tools (create_file / edit_file /
    /// edit_file_lines / rewrite_file), or a code-sandbox version-back. `id` is
    /// the stable file_id; the recipient refetches the file + its versions (if
    /// open in a panel).
    File,

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
    /// Admin view of an authentication provider (Google/Microsoft/Apple/LDAP
    /// /OIDC). Emitted on create/update/test/delete and on auto-disable. The
    /// public `/api/auth/providers` (login page) is unaffected — it just shows
    /// the next list state on the next page load.
    AuthProvider,
    /// Deployment-wide summarization settings (singleton). Notify-only;
    /// the frontend refetches via the existing REST endpoint.
    SummarizationAdminSettings,
    /// Deployment-wide web search settings + provider config (singleton).
    /// Notify-only; the frontend refetches settings + the provider catalog.
    WebSearchSettings,

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

    // --- Phase 8: skills + workflows (see plan §3 + §4.4) ---
    /// A user-installed skill (user-scope). Notify-only — the client refetches
    /// `/api/skills` (which returns the user's own + accessible system skills).
    Skill,
    /// Admin view of a system-scope skill (assigned via group_skills).
    /// Emitted ALONGSIDE `Skill` when scope='system' to refresh both surfaces.
    SkillSystem,
    /// A user-installed workflow (user-scope). Same shape as Skill.
    Workflow,
    /// Admin view of a system-scope workflow.
    WorkflowSystem,
    /// A workflow_runs lifecycle transition (started / completed / failed /
    /// cancelled) — NOT per-step events; those go on the dedicated per-run
    /// SSE channel (§4.4). Notify-only so cross-device list views refresh.
    WorkflowRun,
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

/// Delivery scope for one event, chosen by the publishing handler. There is
/// NO central per-entity table: the module that owns the mutation decides who
/// may learn of it, using its OWN typed permissions. Build it with the typed
/// constructors below so a renamed/removed permission is a compile error.
#[derive(Debug, Clone)]
pub enum Audience {
    /// Only the owning user's connections.
    Owner(Uuid),
    /// Only connections whose permission snapshot satisfies the rule
    /// (admins always qualify).
    Perm(PermRule),
    /// Every authenticated connection.
    Everyone,
}

/// A composable permission requirement (mirrors the frontend `PermissionExpr`).
#[derive(Debug, Clone)]
pub enum PermRule {
    /// The connection must hold EVERY listed permission.
    All(Vec<&'static str>),
    /// The connection must hold AT LEAST ONE listed permission.
    Any(Vec<&'static str>),
}

impl Audience {
    /// Deliver only to `user_id`'s own connections.
    pub fn owner(user_id: Uuid) -> Self {
        Audience::Owner(user_id)
    }

    /// Deliver to every authenticated connection.
    pub fn everyone() -> Self {
        Audience::Everyone
    }

    /// Deliver to holders of a single typed permission, e.g.
    /// `Audience::perm::<LlmModelsRead>()`.
    pub fn perm<P: PermissionCheck>() -> Self {
        Audience::Perm(PermRule::All(vec![P::PERMISSION]))
    }

    /// Deliver to holders of ALL permissions in the tuple, e.g.
    /// `Audience::all_of::<(LlmProvidersRead, LlmModelsRead)>()`. Reuses the
    /// same `PermissionList` tuple machinery as `RequirePermissions<(A, B)>`.
    pub fn all_of<L: PermissionList>() -> Self {
        Audience::Perm(PermRule::All(L::permissions()))
    }

    /// Deliver to holders of ANY permission in the tuple, e.g.
    /// `Audience::any_of::<(McpServersRead, McpServersAdminRead)>()`.
    pub fn any_of<L: PermissionList>() -> Self {
        Audience::Perm(PermRule::Any(L::permissions()))
    }
}

/// Publish a change notification to the audience the caller chose.
///
/// The publishing module decides the `audience` (owner / permission rule /
/// everyone) — this core has no per-entity policy. `origin_conn` is the
/// originating SSE connection, skipped to suppress self-echo.
pub fn publish(
    entity: SyncEntity,
    action: SyncAction,
    id: Uuid,
    audience: Audience,
    origin_conn: Option<Uuid>,
) {
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

    struct PermA;
    impl PermissionCheck for PermA {
        const NAME: &'static str = "PermA";
        const PERMISSION: &'static str = "a::read";
        const DESCRIPTION: &'static str = "";
        const MODULE: &'static str = "test";
    }
    struct PermB;
    impl PermissionCheck for PermB {
        const NAME: &'static str = "PermB";
        const PERMISSION: &'static str = "b::read";
        const DESCRIPTION: &'static str = "";
        const MODULE: &'static str = "test";
    }

    #[test]
    fn perm_constructor_carries_the_typed_permission_string() {
        match Audience::perm::<PermA>() {
            Audience::Perm(PermRule::All(ps)) => assert_eq!(ps, vec!["a::read"]),
            other => panic!("expected Perm(All), got {other:?}"),
        }
    }

    #[test]
    fn all_of_and_any_of_collect_the_permission_tuple() {
        match Audience::all_of::<(PermA, PermB)>() {
            Audience::Perm(PermRule::All(ps)) => assert_eq!(ps, vec!["a::read", "b::read"]),
            other => panic!("expected Perm(All), got {other:?}"),
        }
        match Audience::any_of::<(PermA, PermB)>() {
            Audience::Perm(PermRule::Any(ps)) => assert_eq!(ps, vec!["a::read", "b::read"]),
            other => panic!("expected Perm(Any), got {other:?}"),
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
