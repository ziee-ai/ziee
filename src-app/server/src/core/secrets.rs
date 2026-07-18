//! Re-export shim — Chunk BG-2.
//!
//! The at-rest secret storage-key process-global moved to `ziee-framework`
//! alongside the `secret` crypto that reads it (so `resolve_optional_secret`
//! keeps its exact signature). It is ONE static per process regardless of which
//! crate owns it; `init_storage_key` (boot) writes it, every `storage_key()`
//! read observes the same value — behaviour byte-identical. This shim keeps
//! every consumer that names `crate::core::secrets::…` byte-unchanged.

pub use ziee_framework::secrets::*;
