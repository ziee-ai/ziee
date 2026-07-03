// Extension registration for the project chat extension.

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

/// Run BEFORE assistant (order 10) and file (order 20) — see project.rs
/// for the layering rationale.
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "project",
    order: 8,
};

/// No client-side fields. Project context is derived server-side from
/// `conversation.project_id`. Letting clients pass `project_id` on a
/// per-send request would allow injecting project Y's context into
/// conversation X — easy mistake; locked out by design.
// Convention: every chat extension declares a `SendMessageRequestFields`
// (Deserialize + JsonSchema); this one is intentionally empty (see above).
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {}

pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::project::ProjectExtension::new(pool))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static PROJECT_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
