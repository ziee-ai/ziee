//! Common test utilities for desktop integration tests

pub mod server;

/// Reserve an unused TCP port for a test server.
///
/// Uses `portpicker::pick_unused_port()` (the same primitive the server
/// crate's `harness_inner.rs` uses) instead of the old bind-to-check-then-
/// release loop, which was TOCTOU-racy: two parallel tests could each see a
/// port as "available", release it, and then both try to bind it → one of
/// them fails with "Address already in use". `pick_unused_port()` reserves a
/// port the OS hands out, eliminating the race window that previously forced
/// `--test-threads=1`.
///
/// The `start`/`end` range parameters are retained for API compatibility but
/// are now advisory only — the OS chooses the port. Returns `None` only if no
/// free port is available at all.
pub fn find_available_port(_start: u16, _end: u16) -> Option<u16> {
    portpicker::pick_unused_port()
}

/// Test configuration for desktop app tests
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub server_port: u16,
    pub data_dir: std::path::PathBuf,
}

impl TestConfig {
    pub fn new(data_dir: std::path::PathBuf) -> Self {
        // Parallel-safe port reservation (see `find_available_port`). Each
        // test passes its own fresh `tempfile::tempdir()` as `data_dir`, so
        // every test already has an isolated mutable state dir (embedded-PG
        // dir, workspace, etc.) — the port pick was the last shared resource
        // that made this harness collide under parallelism.
        let server_port =
            portpicker::pick_unused_port().expect("No available TCP port found");
        Self {
            server_port,
            data_dir,
        }
    }
}
