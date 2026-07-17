//! Chat host for the shared `agent_core::AgentCore` loop (ITEM-24/25/26, full
//! extension re-home). The six chat-flavored port impls live in the submodules;
//! `ChatAgentDispatcher` (added at fan-in) assembles + runs the core.
//!
//! Integration contract (pin): tool attachment flows through
//! `TurnContext.tool_scope.servers` — each ported context-injector extension
//! pushes its built-in server NAME there in `contribute`; `ChatToolProvider::list`
//! gathers tools from `tool_scope.servers` (mirrors the workflow McpToolProvider).

pub mod dispatcher;
pub mod event_sink;
pub mod gate;
pub mod resolver;
pub mod transcript;
