//! Chat host for the shared `agent_core::AgentCore` loop (ITEM-24/25/26, full
//! extension re-home). The six chat-flavored port impls live in the submodules;
//! `ChatAgentDispatcher` (added at fan-in) assembles + runs the core.
//!
//! Integration contract (pin): for the CHAT host, the tool set the model sees is
//! authored by the real MCP chat-extension: `RegistryBridge::before_model` runs the
//! extension's `before_llm_call`, which sets `request.tools = <all attached tools>`
//! namespaced as `<server_id>__<tool>` (uuid scheme — see `ChatToolProvider::call`
//! /`is_trusted`, which parse that prefix). `ChatToolProvider::list` (invoked by the
//! crate loop before `before_model`) mirrors the workflow `McpToolProvider` shape but
//! its result is OVERWRITTEN by that `before_llm_call` each iteration, so it is NOT
//! the source of the chat model's tool list — do NOT edit `list` expecting to change
//! what the model sees; change the chat extension. (`list` still opens/pools the
//! per-server session it will reuse for execution.) The `<server_name>__<tool>` NAME
//! scheme in the doc below applies to the WORKFLOW host, not chat.

pub mod dispatcher;
pub mod event_sink;
pub mod gate;
pub mod registry_bridge;
pub mod resolver;
pub mod transcript;
pub mod uniquify;
