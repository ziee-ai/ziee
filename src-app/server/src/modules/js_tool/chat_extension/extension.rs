//! js_tool extension registration for the chat module.

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "js_tool",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_run_js_mcp` metadata flag, which the MCP extension reads in
    // `auto_attach_builtin_ids` when building the tool list. 29 lands it after
    // the other attach-flag extensions (web_search 26 … citations 29), before
    // MCP (30). The nudge is generic (points the model at `ziee.toolList()`), so
    // it needs no knowledge of the assembled tool set and the tie with citations
    // (also 29) is benign.
    order: 29,
};

pub fn create(pool: PgPool, config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::js_tool::JsToolExtension::new(pool, config))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static JS_TOOL_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
