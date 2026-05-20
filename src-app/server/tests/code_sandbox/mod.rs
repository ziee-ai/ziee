// Phase 9 — comprehensive automated coverage for code_sandbox.
//
// Tier 1 (pure unit tests) live as `#[cfg(test)] mod tests` blocks
// alongside the implementations in src/modules/code_sandbox/. They
// run with `cargo test --lib`. 33 tests today.
//
// This module gathers the higher tiers that need a live server:
//   Tier 2 — DB integration tests (Postgres).
//   Tier 3 — HTTP handler + concurrency tests.
//   Tier 4 — bwrap-required tests (`#[ignore]`d).
//
// Tier 5 (real-LLM chat-level) lives under tests/chat/ alongside the
// existing mcp_*_test.rs files; not gathered here.

mod tier2_repository;
mod tier2_migrations;
mod tier2_mcp_listing;
mod tier3_http;
mod tier3_concurrency;
mod tier4_sandbox_smoke;
mod tier4_hardening;
