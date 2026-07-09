// Integration tests for ziee backend API
// Each module contains tests for specific functionality

mod agentic_chat;
mod assistant;
mod auth;
mod bio_mcp;
mod chat;
mod citations;
mod code_sandbox;
mod control_mcp;
mod common;
mod elicitation_mcp;
mod file;
mod file_rag;
mod files_mcp;
mod hardware;
mod health;
mod hub;
mod knowledge_base;
mod lit_search;
mod llm_local_runtime;
mod llm_model;
mod llm_provider;
mod llm_provider_files;
mod llm_repository;
mod mcp;
mod memory;
mod memory_mcp;
mod onboarding;
mod project;
mod server_update;
mod skill;
mod summarization;
mod sync;
mod tool_result_mcp;
mod web_search;
mod workflow;
mod workflow_mcp;
// `remote_access` integration tests live in the desktop crate now —
// they exercise endpoints served only by the ziee-desktop binary
// and are physically located at
// `desktop/tauri/tests/remote_access/`. Run via
// `cd src-app/desktop/tauri && cargo test --test integration_tests`.
mod user;
