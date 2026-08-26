//! code_sandbox attach-extension registration for the chat module.

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "code_sandbox_attach",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_code_sandbox` metadata flag, which the MCP extension reads in
    // `auto_attach_builtin_ids` when building the tool list. 21 lands it before
    // the MCP collector (30) and alongside the other built-in attach extensions
    // (file 20, control 22, web_search 26, …). If it ran at ≥30 the flag would be
    // set after MCP already built its tools and `execute_command` would never
    // attach.
    order: 21,
};

pub fn create(_pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::CodeSandboxAttachExtension)
}

#[distributed_slice(CHAT_EXTENSIONS)]
static CODE_SANDBOX_ATTACH_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
