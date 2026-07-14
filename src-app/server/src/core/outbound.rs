//! App-side adapter for the generic SSRF-validated outbound-HTTP helpers —
//! Chunk BG.
//!
//! `crate::utils::url_validator` is **domain-free framework infrastructure**
//! (a pure URL/IP allowlist + validated-`reqwest`-client builder), not an
//! app-global singleton like `Repos` / `EventBus`. The right long-term home is
//! `ziee-framework`; that move is deferred here because this in-ziee refactor
//! must not touch the SDK submodule.
//!
//! This module re-exports the three entry points the auth providers use, so
//! `modules::auth::providers::{oauth2,apple}` name `crate::core::outbound::…`
//! instead of `crate::utils::url_validator::…`. That removes the direct
//! `url_validator` coupling from the auth module (which otherwise blocks the
//! `ziee-auth` extraction) while keeping behaviour byte-identical — these are the
//! same functions, re-exported.
//!
//! Chunk BG-2: `url_validator` has now landed in `ziee-framework`, so this
//! adapter points straight at the framework crate (`ziee_framework::url_validator`)
//! rather than the local `crate::utils::url_validator` shim.

pub use ziee_framework::url_validator::{
    OutboundUrlPolicy, build_validated_client, validate_outbound_url,
};
