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

pub mod runtime;

// The remaining submodules (host_bridge, approval, executor, limits, handlers,
// routes, tools, repository, permissions, chat_extension) + the `AppModule`
// registration land in subsequent lifecycle steps; `runtime` is the
// self-contained, unit-tested foundation.
