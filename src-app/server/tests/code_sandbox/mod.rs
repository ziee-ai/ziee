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

pub mod harness;
pub mod mirror_fixture;

mod tier2_repository;
mod tier2_migrations;
mod tier2_mcp_listing;
mod tier2_built_in_protection;
mod tier2_workspace_reaper;
mod tier3_http;
mod tier3_concurrency;
mod tier3_versions;
mod tier3_resource_limits;
mod tier4_sandbox_smoke;
mod tier4_hardening;
mod tier4_pid_ns_fallback;
mod tier4_cgroup_fallback;
mod tier4_seccomp;

// Tier 6 — full HTTP-E2E suite: boots a real TestServer with
// code_sandbox enabled, posts real JSON-RPC, the handler runs real
// bwrap with the production argv, and the response carries the real
// command output. All `#[ignore]`'d; requires rootfs mounted +
// bwrap installed (the harness::enabled_test_server() helper skips
// cleanly when either is missing).
mod tier6_http_e2e;
mod tier6_mcp_sandbox_e2e;
mod tier6_security_regression;
mod tier6_hardening;
mod tier6_version_swap;

// Tier 8 — TRULY-PUBLISHED MCP package smoke. Pip-installs
// `mcp-server-fetch` from PyPI into the sandbox, then exec's it via
// python3 -m. Real network egress to https://example.com to assert
// fetch works end-to-end. Rootfs + network-gated. `#[ignore]`'d.
mod tier8_real_mcp_package;
