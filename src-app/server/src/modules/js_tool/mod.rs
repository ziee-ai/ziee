//! `js_tool` — provider-agnostic **programmatic tool calling** (`run_js`).
//!
//! A new built-in tool `run_js(script)` where ANY model writes JavaScript that
//! executes in an EMBEDDED QuickJS interpreter IN-PROCESS, with the
//! conversation's MCP tools injected as async host functions
//! (`await ziee.tools.web_search({query})`). Intermediate sub-tool results stay
//! inside the running script; only the script's FINAL value returns to the
//! model's context — giving PTC token economics for every provider.
//!
//! Why embedded (not code_sandbox): code_sandbox's mac/windows backends cross a
//! VM boundary and `--clearenv` the environment, so a live host function that
//! re-enters the in-process MCP dispatcher is impossible there by construction.
//! An embedded interpreter is cross-platform in-process everywhere, needs NO
//! credential (the injected host function IS the capability), has NO ambient
//! fs/net/env, and its host-fn calls land in the existing dispatcher chokepoint
//! so per-call APPROVAL + `mcp_tool_calls` RECORDING just work — including
//! suspending the script in-process while awaiting a user approval.
//!
//! Layout (mirrors `memory_mcp/` for registration; the runtime/bridge/approval
//! are the novel core):
//! - `runtime`     — pure embedded-interpreter wrapper + caps (no chat context).
//! - `host_bridge` — injects `ziee.tools.*` re-entering the MCP dispatcher.
//! - `approval`    — per-call approval suspend/resume (elicitation oneshot).
//! - `executor`    — the entry `mcp.rs` calls; wires the three together.
//! - `limits`      — the configurable caps.
//! - `mod`/`repository`/`routes`/`handlers`/`tools` — the built-in server row.
//! - `permissions` — `js_tool::use`.
//! - `chat_extension` — the attach flag + system nudge.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod approval;
pub mod chat_extension;
pub mod executor;
pub mod handlers;
pub mod host_bridge;
pub mod limits;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod runtime;
pub mod settings;
pub mod settings_cache;
pub mod tools;

pub use repository::JsToolRepository;

/// Deterministic UUID for the built-in `run_js` MCP server row. Stable across
/// deployments (mirrors `memory_mcp_server_id`).
pub fn run_js_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"run_js.ziee.internal")
}

/// Is the js_tool feature enabled for this deployment (config kill switch)?
pub fn is_enabled(config: &crate::core::config::Config) -> bool {
    config.js_tool.as_ref().map(|c| c.enabled).unwrap_or(true)
}

#[distributed_slice(MODULE_ENTRIES)]
static JS_TOOL_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "js_tool",
    // After mcp (65) so the mcp_servers table is initialized.
    order: 90,
    description: "Built-in run_js tool (programmatic tool calling in an embedded JS runtime)",
    constructor: || Box::new(JsToolModule::new()),
};

pub struct JsToolModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl JsToolModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for JsToolModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for JsToolModule {
    fn name(&self) -> &'static str {
        "js_tool"
    }

    fn description(&self) -> &'static str {
        "Built-in run_js tool (programmatic tool calling in an embedded JS runtime)"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Deploy-level kill switch (default enabled) — mirrors web_search /
        // lit_search. When off, the server row is never registered so the chat
        // extension never attaches run_js.
        if !is_enabled(&ctx.config) {
            tracing::info!("js_tool: disabled by config (js_tool.enabled=false); run_js not registered");
            return Ok(());
        }

        // Pin loopback (defense-in-depth) so the built-in's JWT-bearing calls
        // can't be redirected off-box (same helper code_sandbox/memory_mcp use).
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!("http://{host}:{port}/api/run-js/mcp", port = ctx.config.server.port);

        let server_id = run_js_mcp_server_id();
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = repository::JsToolRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &loopback_url).await {
                Ok(()) => tracing::info!("js_tool: built-in run_js server {server_id} registered at {loopback_url}"),
                Err(e) => tracing::error!("js_tool: upsert_builtin_server failed: {e:?}"),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::js_tool_router())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module_api::AppModule;

    // TEST-17: the built-in server id is deterministic + distinct from peers.
    #[test]
    fn server_id_is_stable_and_distinct() {
        assert_eq!(run_js_mcp_server_id(), run_js_mcp_server_id());
        assert!(!run_js_mcp_server_id().is_nil());
        assert_ne!(
            run_js_mcp_server_id(),
            crate::modules::memory_mcp::memory_mcp_server_id()
        );
    }

    // TEST-29: the module + its AppModule impl are present.
    #[test]
    fn module_present() {
        assert_eq!(JsToolModule::new().name(), "js_tool");
    }

    // TEST-26: config defaults to enabled; an explicit false disables.
    #[test]
    fn config_default_enabled() {
        assert!(crate::core::config::JsToolConfig::default().enabled);
        assert!(!crate::core::config::JsToolConfig { enabled: false }.enabled);
    }
}
