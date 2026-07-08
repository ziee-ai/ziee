//! Integration tests that exercise endpoints served by the
//! `ziee-desktop` binary in headless mode.
//!
//! The test harness (`TestServer` + helpers) is shared with the
//! server crate's integration_tests binary via `#[path]` — the
//! harness is binary-agnostic (it can spawn either `ziee` or
//! `ziee-desktop --headless`), so duplicating it would just create
//! drift.
//!
//! Run via `cargo test --test integration_tests` from this crate
//! (or `just check-remote-access-unit`).

// Use the slim harness file (no OAuth/LDAP/Apple mock declarations),
// so we don't drag in wiremock/ldap3/rsa/jsonwebtoken/testcontainers
// just to spawn the desktop binary.
#[path = "../../../server/tests/common/harness_inner.rs"]
mod common;

mod auto_assign_mcp;
mod backend_lifecycle;
mod host_mount_tests;
mod remote_access;
mod office_bridge;
