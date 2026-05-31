//! Server-crate test harness entrypoint.
//!
//! The TestServer + test_helpers are factored out into
//! `harness_inner.rs` so the desktop crate can reuse them via
//! `#[path]` without dragging in the heavy OAuth/LDAP/Apple mock
//! deps that only server tests need.

pub mod apple_mock;
pub mod ldap_mock;
pub mod oauth_mock;

#[path = "harness_inner.rs"]
mod inner;
pub use inner::*;
