//! Re-export shim — Chunk BG-2.
//!
//! The at-rest secret crypto (`encrypt_secret` / `decrypt_secret` /
//! `resolve_optional_secret` + `SecretView` / `mask_secret`) moved to
//! `ziee-framework` (RATIFIED SDK home; build-DB-free — runtime `query_as`, never
//! a compile-time `query!`). This shim keeps every consumer that names
//! `crate::common::secret::…` byte-unchanged. `SecretView` has no `JsonSchema`
//! impl, so nothing here touches the OpenAPI wire surface.

pub use ziee_framework::secret::*;
