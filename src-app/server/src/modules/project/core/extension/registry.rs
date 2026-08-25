// Project extension infrastructure
//
// Mirrors the proven `CHAT_EXTENSIONS` pattern at
// `modules/chat/core/extension/registry.rs` — sibling modules register
// themselves via the `PROJECT_EXTENSIONS` distributed slice without the
// project module having to import them. Stripped to just routes +
// lifecycle hooks; project doesn't stream, so there's no SSE / delta /
// content-block surface.
//
// Acid-test invariant: deleting any extension module (e.g. file) must
// leave the project module compiling and running normally — the
// distributed slice simply collects zero entries from that module.


use aide::axum::ApiRouter;
use ai_providers::ContentBlock;
use async_trait::async_trait;
use linkme::distributed_slice;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;

use crate::common::AppError;
use ziee_framework::entity_extension::{
    ExtensionRegistry as GenericExtensionRegistry, ExtensionRegistrySingleton,
};

/// Extension registration entry for distributed collection.
///
/// Now an alias over the generic `ziee_framework` primitive (gap G8): the
/// `{name, order, factory}` shape + the `#[distributed_slice]` mechanics are
/// domain-agnostic and shared with the chat registry (and CytoAnalyst's
/// `study`). Sibling modules still construct `ProjectExtensionEntry { name,
/// order, factory }` via this alias — unchanged.
pub type ProjectExtensionEntry =
    ziee_framework::ExtensionEntry<dyn ProjectExtension, Arc<crate::core::config::Config>>;

/// Distributed slice for collecting all project extensions.
///
/// Sibling modules register via `#[distributed_slice(PROJECT_EXTENSIONS)]`
/// on a static of type `ProjectExtensionEntry`. The project module's
/// auto-register collects + sorts by order at startup. Empty slice (no
/// extensions registered) is a valid runtime state — project still works.
#[distributed_slice]
pub static PROJECT_EXTENSIONS: [ProjectExtensionEntry] = [..];

/// Extension trait for project functionality.
///
/// Project extensions contribute two things today:
/// 1. **Routes** — additional API endpoints mounted into the public API
///    router (e.g. file extension's `/api/projects/{id}/files*`).
/// 2. **Lifecycle hooks** — synchronous in-transaction hooks for project
///    lifecycle events that need per-extension data fan-out (e.g. file
///    extension cloning `project_files` rows on duplicate).
///
/// No `on_project_deleted` hook is provided — the schema-level
/// `ON DELETE CASCADE` on join tables (e.g. `project_files`) handles
/// cleanup at the database layer. If a future extension needs
/// side-effect cleanup beyond CASCADE, prefer an event-bus listener.
#[async_trait]
pub trait ProjectExtension: Send + Sync {
    /// Extension name (for logging and debugging).
    fn name(&self) -> &str;

    /// Register custom routes for this extension.
    ///
    /// Called by `ProjectExtensionRegistry::register_routes` during the
    /// project module's `register_routes`. Extensions can mount routes
    /// anywhere in the URL space — the file extension uses this to
    /// register `/api/projects/{id}/files*` while living in the file
    /// module. Default: no routes.
    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router
    }

    /// Called inside the project-duplicate transaction.
    ///
    /// Allows extensions to clone their per-project state (e.g. file
    /// extension clones `project_files` rows from src to dst). Errors
    /// abort the entire duplicate — the project row insert and all
    /// extension hooks share the same transaction.
    ///
    /// Default: no-op. Override when your extension owns per-project
    /// rows that should be carried into duplicates.
    async fn on_project_duplicated(
        &self,
        _src_project_id: Uuid,
        _dst_project_id: Uuid,
        _tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    /// Collect this extension's chat-context contribution for a project.
    ///
    /// Called by the project's chat extension (`before_llm_call` path)
    /// so each knowledge-kind extension can resolve its per-project
    /// content into ready-to-inject `ContentBlock`s. The project chat
    /// extension concatenates contributions from all extensions and
    /// injects them ahead of the user message alongside the project's
    /// instructions.
    ///
    /// The file extension implements this by:
    ///   1. Listing attached files for the project.
    ///   2. Routing each file through provider-specific `process_file_blocks`.
    ///   3. Wrapping the result in `[Project knowledge file: <name>]`
    ///      markers so the LLM can attribute the source.
    ///
    /// `provider_id` + `provider_type` come from the chat stream context's
    /// metadata — extensions that don't route through providers can ignore
    /// them.
    ///
    /// Default: contribute nothing.
    async fn collect_chat_knowledge(
        &self,
        _project_id: Uuid,
        _user_id: Uuid,
        _provider_id: Uuid,
        _provider_type: &str,
    ) -> Result<Vec<ContentBlock>, AppError> {
        Ok(Vec::new())
    }

    /// Called inside the conversation-attach transaction, after the
    /// project_conversations row insert. Lets extensions sync per-
    /// conversation state from the project. Mcp uses this to snapshot
    /// the project's mcp_settings row into a new conversation-scoped
    /// row (INSERT…SELECT on the unified mcp_settings table with a
    /// different FK). Errors abort the attach.
    ///
    /// Default: no-op.
    async fn on_conversation_attached(
        &self,
        _project_id: Uuid,
        _conversation_id: Uuid,
        _user_id: Uuid,
        _tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    /// Called inside the conversation-detach transaction, after the
    /// project_conversations row delete. Lets extensions clean up per-
    /// conversation state (mcp deletes the conversation's mcp_settings
    /// row so chat falls back to user/global defaults). Conversation
    /// deletion is handled by ON DELETE CASCADE on the FK; this hook
    /// covers detach-but-keep-conversation.
    ///
    /// Default: no-op.
    async fn on_conversation_detached(
        &self,
        _conversation_id: Uuid,
        _tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    /// Initialize extension (called once at startup).
    // Trait lifecycle hook (default no-op); driven by `initialize_all`, which
    // the project module runs once at startup (see project/mod.rs::init).
    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        Ok(())
    }
}

/// Registry for managing project extensions.
///
/// A thin newtype over the generic `ziee_framework` registry primitive (gap
/// G8): the storage + `register` + `iter` + route-fold + in-tx fan-out
/// mechanics are shared; only these domain fan-out methods (which name
/// project-specific hooks) live here. The route fold delegates to
/// `fold_routes`; every in-transaction hook delegates to `fire_in_tx` (the same
/// combinator that offers the optional `on_<entity>_deleted` hook — project is
/// cascade-only, so it never uses that one).
pub struct ProjectExtensionRegistry(GenericExtensionRegistry<dyn ProjectExtension>);

impl ProjectExtensionRegistry {
    pub fn new() -> Self {
        Self(GenericExtensionRegistry::new())
    }

    /// Register an extension.
    pub fn register(&mut self, extension: Arc<dyn ProjectExtension>) {
        tracing::info!("Registering project extension: {}", extension.name());
        self.0.register(extension);
    }

    /// Initialize all registered extensions. Run once at startup by
    /// `project/mod.rs::init`, driving each extension's `initialize` hook.
    pub async fn initialize_all(&self, pool: &PgPool) -> Result<(), AppError> {
        for ext in self.0.iter() {
            ext.initialize(pool).await?;
        }
        Ok(())
    }

    /// Fold every extension's routes into the given router.
    ///
    /// Delegates to the generic `fold_routes` combinator. Extensions
    /// that don't register routes are no-ops (default trait impl).
    pub fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        self.0
            .fold_routes(router, |router, ext| ext.register_routes(router))
    }

    /// Call `on_project_duplicated` on all extensions sequentially.
    ///
    /// Sequential rather than concurrent because each extension shares
    /// the same `&mut Transaction`. First error aborts the iteration
    /// and bubbles up — the caller's `tx.commit()` is then never
    /// reached, so the duplicate fails atomically. Delegates to the
    /// generic `fire_in_tx` shared-transaction combinator.
    pub async fn fire_on_project_duplicated(
        &self,
        src_project_id: Uuid,
        dst_project_id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        self.0
            .fire_in_tx(tx, |ext, tx| {
                ext.on_project_duplicated(src_project_id, dst_project_id, tx)
            })
            .await
    }

    /// Call `on_conversation_attached` on all extensions sequentially.
    /// Atomic with the project_conversations INSERT (shared transaction).
    pub async fn fire_on_conversation_attached(
        &self,
        project_id: Uuid,
        conversation_id: Uuid,
        user_id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        self.0
            .fire_in_tx(tx, |ext, tx| {
                ext.on_conversation_attached(project_id, conversation_id, user_id, tx)
            })
            .await
    }

    /// Call `on_conversation_detached` on all extensions sequentially.
    /// Atomic with the project_conversations DELETE (shared transaction).
    pub async fn fire_on_conversation_detached(
        &self,
        conversation_id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        self.0
            .fire_in_tx(tx, |ext, tx| ext.on_conversation_detached(conversation_id, tx))
            .await
    }

    /// Collect chat-knowledge contributions from every extension.
    ///
    /// Sequential to keep per-extension errors deterministic (and the
    /// fan-out is small — one extension per knowledge kind, typically
    /// < 5). The project chat extension calls this once per
    /// `before_llm_call` to assemble all knowledge into the LLM
    /// context.
    pub async fn collect_chat_knowledge(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        provider_id: Uuid,
        provider_type: &str,
    ) -> Result<Vec<ContentBlock>, AppError> {
        let mut all = Vec::new();
        for ext in self.0.iter() {
            let blocks = ext
                .collect_chat_knowledge(project_id, user_id, provider_id, provider_type)
                .await?;
            all.extend(blocks);
        }
        Ok(all)
    }
}

impl Default for ProjectExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-wide singleton for the project-extension registry.
///
/// Set once by `ProjectModule::init` (after `auto_register_project_extensions`
/// runs) and read by code paths that need extension fan-out but don't
/// receive the registry via axum Extension — specifically the project
/// chat-extension's `before_llm_call`, which runs from inside chat's
/// streaming pipeline and never sees the project router's Extension
/// layer.
///
/// Mirrors the `Repos` global-singleton pattern. Returns `None` if
/// accessed before module init (e.g. in tests that bypass the standard
/// boot sequence) — callers should handle that gracefully.
///
/// Backed by the generic `ziee_framework` singleton primitive (gap G8). Chat's
/// registry needs no singleton (it's always threaded through axum `Extension`);
/// project's chat-extension runs from inside chat's streaming pipeline, which
/// never sees the project router's layers, so it reads this global.
static PROJECT_EXTENSION_REGISTRY: ExtensionRegistrySingleton<ProjectExtensionRegistry> =
    ExtensionRegistrySingleton::new();

pub fn set_global_registry(registry: Arc<ProjectExtensionRegistry>) {
    PROJECT_EXTENSION_REGISTRY.set(registry);
}

pub fn get_global_registry() -> Option<Arc<ProjectExtensionRegistry>> {
    PROJECT_EXTENSION_REGISTRY.get()
}
