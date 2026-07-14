//! Server-crate test harness entrypoint.
//!
//! The TestServer + test_helpers are factored out into
//! `harness_inner.rs` so the desktop crate can reuse them via
//! `#[path]` without dragging in the heavy OAuth/LDAP/Apple mock
//! deps that only server tests need.

pub mod chat_stream_probe;
pub mod oai_capture_stub;
pub mod stub_chat;
pub mod stub_engine;

// Chunk sdk-test-fixtures: the 4 GENERIC auth/sync mocks moved into the SDK's
// `ziee-test-harness` (feature `fixtures`) so a second app can drive the same
// flows. Re-exported under their original module paths so every call site
// (`crate::common::oauth_mock::…`, `crate::common::sync_probe::SyncProbe`, …)
// compiles UNCHANGED.
pub mod apple_mock {
    pub use ziee_test_harness::fixtures::apple_mock::*;
}
pub mod ldap_mock {
    pub use ziee_test_harness::fixtures::ldap_mock::*;
}
pub mod oauth_mock {
    pub use ziee_test_harness::fixtures::oauth_mock::*;
}
pub mod sync_probe {
    pub use ziee_test_harness::fixtures::sync_probe::*;
}

#[path = "harness_inner.rs"]
mod inner;
pub use inner::*;

// The `SyncProbe::open` seam: our `TestServer` shim supplies the
// `/sync/subscribe` URL so the moved (app-neutral) probe never names an
// app-side type. `self.api_url(path)` resolves to `TestServer`'s inherent
// method (inherent methods win over the same-named trait method — no recursion).
impl ziee_test_harness::ApiUrlTarget for TestServer {
    fn api_url(&self, path: &str) -> String {
        self.api_url(path)
    }
}
