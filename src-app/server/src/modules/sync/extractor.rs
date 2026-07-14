//! `SyncOrigin` request extractor — moved to `ziee_framework::sync` in chunk B5
//! (app-agnostic: reads the `X-Sync-Connection-Id` header for self-echo
//! suppression). Re-exported here so `crate::modules::sync::SyncOrigin` (and
//! `super::extractor::SyncOrigin`) resolve unchanged at every call site.

pub use ziee_framework::sync::SyncOrigin;
