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
    /// A user's saved web_search provider key changed (set/clear). Owner-scoped;
    /// notify-only — the client refetches `GET /api/web-search/user-keys` (masked
    /// state only). `id` is `Uuid::nil()` (the key is addressed by provider name,
    /// not a uuid; the refetch reloads the whole per-user catalog).
    WebSearchUserKey,
    /// A user's saved lit_search connector key changed (set/clear). Owner-scoped;
    /// notify-only — the client refetches `GET /api/lit-search/user-keys`.
    LitSearchUserKey,
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
    /// A recorded MCP tool-call invocation (`mcp_tool_calls`). Emitted
    /// owner-scoped on Create from the detached recording task in
    /// `McpSession::call_tool`, so the per-server "Calls" tab refreshes live.
    /// Notify-only; the client refetches via `GET /api/mcp/tool-calls`.
    McpToolCall,
    /// A user-owned file's RAG index lifecycle state changed
    /// (`file_index_state`: pending/indexing/indexed/failed/no_text). Emitted
    /// owner-scoped from the `file_rag` ingest path on each transition so the
    /// knowledge-base documents UI reflects per-doc indexing status live. `id`
    /// is the file_id; the client refetches the KB's document status.
    FileIndexState,
    /// A user-owned knowledge base changed (create/rename/delete or its document
    /// set). Owner-scoped; the client refetches the KB list / detail.
    KnowledgeBase,
    /// A document within a knowledge base changed (attach/detach/status). Owner-
    /// scoped; `id` is the knowledge_base id; the client refetches its documents.
    KnowledgeBaseDocument,
    /// The caller's own default MCP settings for new conversations changed
    /// (approval mode / auto-approved tools / disabled servers / loop
    /// settings). Owner-scoped; notify-only — the client refetches
    /// `GET /api/mcp/defaults` (gated `conversations::read`). `id` is
    /// `Uuid::nil()` (a per-user singleton addressed by owner, not a uuid).
    McpDefaults,
    /// A conversation's deliverables curation changed (a file pinned/hidden as a
    /// deliverable). Owner-scoped; notify-only — the client refetches
    /// `GET /api/conversations/{id}/deliverables`. `id` is the conversation id.
    Deliverable,

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
    /// Deployment-wide Document-RAG (file_rag) admin settings (singleton).
    FileRagAdminSettings,
    /// A user's Letta-style assistant core-memory blocks changed (owner-scoped).
    AssistantCoreMemory,
    /// Code-sandbox resource-limit settings (singleton).
    CodeSandboxSettings,
    /// run_js (js_tool) resource-limit settings (singleton, admin-scoped).
    JsToolSettings,
    /// Code-sandbox rootfs version list changed (install/evict/delete).
    CodeSandboxRootfsVersion,
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
    /// Deployment-wide JWT session settings (singleton): access-token TTL +
    /// max session length. Notify-only; the admin UI refetches via
    /// `GET /api/auth/session-settings`.
    SessionSettings,
    /// Deployment-wide web search settings + provider config (singleton).
    /// Notify-only; the frontend refetches settings + the provider catalog.
    WebSearchSettings,
    /// Deployment-wide literature search settings + connector config (singleton).
    /// Notify-only; the frontend refetches settings + the connector catalog.
    LitSearchSettings,
    /// Deployment-wide voice dictation settings (singleton). Notify-only;
    /// delivered to holders of `voice::admin::read` — the admin UI refetches
    /// `GET /api/voice/settings`.
    VoiceSettings,
    /// Voice whisper-server runtime version list changed (install/delete/set-default).
    /// Notify-only; delivered to holders of `voice::admin::read`.
    VoiceRuntimeVersion,
    /// Deployment-wide MCP user policy (singleton `mcp_user_policy`): which
    /// transports regular users may install + the enforced stdio sandbox
    /// flavor + tool-call retention. Delivered to holders of `mcp_servers::read`
    /// (the read-perm gating `GET /api/mcp/user-policy`); notify-only — each
    /// recipient refetches the sanitized policy. `id` is `Uuid::nil()`.
    McpUserPolicy,
    /// Deployment-wide scheduler admin settings (singleton): per-user task
    /// quota, cadence floor, failure cap, notification retention. Delivered to
    /// holders of `scheduler::admin::read`; notify-only — the admin UI refetches
    /// `GET /api/scheduler/admin-settings`. `id` is `Uuid::nil()`.
    SchedulerAdminSettings,

    /// A user's bibliography library entry changed (add/import/verify/delete).
    /// Owner-scoped; notify-only — the client refetches `/api/citations`.
    BibliographyEntry,

    /// An open Office document was opened or closed on the user's desktop
    /// (the `office_bridge` watch loop diffs successive `list_open_documents`
    /// snapshots). Owner-scoped (DEC-7 — `Audience::owner(user_id)`, never
    /// `everyone()`); notify-only — the client refetches the open-documents
    /// list (self-gated on `office_bridge::use`). `id` is a stable
    /// `Uuid::new_v5` derived from the document's `full_name`, so
    /// open/close of the same document address the same entity id.
    OfficeDocument,

    /// A user's scheduled task changed (create/update/enable/pause/delete) or a
    /// firing advanced its run history. Owner-scoped; notify-only — the client
    /// refetches `/api/scheduled-tasks` (and, if open, the task's runs).
    ScheduledTask,
    /// A new (or updated/read) notification for the user. Owner-scoped;
    /// emitted with origin=None from the background firing so every device
    /// refetches `/api/notifications` (+ the unread count). `id` is the
    /// notification id (or `Uuid::nil()` for a bulk read-all/prune).
    Notification,

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
    /// The caller's onboarding progress changed (a guide / step completed).
    /// Owner-scoped, notify-only — other devices refetch `/api/onboarding/me`
    /// so a guide completed on one device doesn't keep showing on another.
    Onboarding,
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
    /// Every authenticated connection. No current prod caller (owner/perm
    /// scoping covers today's entities); retained as intentional API surface.
    #[allow(dead_code)]
    Everyone,
}

/// A composable permission requirement
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

    /// Deliver to every authenticated connection. Part of the audience API for
    /// genuinely-public entities; no current caller (owner/perm scoping covers
    /// today's entities), so retained as intentional API surface.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn all_of<L: PermissionList>() -> Self {
        Audience::Perm(PermRule::All(L::permissions()))
    }

    /// Deliver to holders of ANY permission in the tuple, e.g.
    /// `Audience::any_of::<(McpServersRead, McpServersAdminRead)>()`.
    #[allow(dead_code)]
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
            (SyncEntity::ScheduledTask, "scheduled_task"),
            (SyncEntity::Notification, "notification"),
            (SyncEntity::SchedulerAdminSettings, "scheduler_admin_settings"),
        ];
        for (e, name) in cases {
            assert_eq!(serde_json::to_string(&e).unwrap(), format!("\"{name}\""));
        }
    }
}

#[cfg(test)]
mod kb_wire_tests {
    use super::SyncEntity;

    // TEST-19 (ITEM-21): the KB sync entities serialize to the exact snake_case
    // wire strings the generated TS `sync:<entity>` keys depend on.
    #[test]
    fn kb_entities_serialize_snake_case() {
        let s = |e: SyncEntity| serde_json::to_value(e).unwrap().as_str().unwrap().to_string();
        assert_eq!(s(SyncEntity::KnowledgeBase), "knowledge_base");
        assert_eq!(s(SyncEntity::KnowledgeBaseDocument), "knowledge_base_document");
        assert_eq!(s(SyncEntity::FileIndexState), "file_index_state");
    }
}
