// Integration tests for ziee backend API
// Each module contains tests for specific functionality

mod agentic_chat;
mod assistant;
mod auth;
mod bio_mcp;
mod chat;
mod code_sandbox;
mod common;
mod file;
mod files_mcp;
mod hardware;
mod hub;
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
mod workflow;
// `remote_access` integration tests live in the desktop crate now —
// they exercise endpoints served only by the ziee-desktop binary
// and are physically located at
// `desktop/tauri/tests/remote_access/`. Run via
// `cd src-app/desktop/tauri && cargo test --test integration_tests`.
mod user;
