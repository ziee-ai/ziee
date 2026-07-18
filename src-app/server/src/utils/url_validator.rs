//! Re-export shim — Chunk BG-2.
//!
//! The SSRF outbound-URL validator moved to `ziee-framework` (its true home;
//! domain-free, build-DB-free framework infra). This shim keeps every consumer
//! that names `crate::utils::url_validator::…` byte-unchanged.

pub use ziee_framework::url_validator::*;
