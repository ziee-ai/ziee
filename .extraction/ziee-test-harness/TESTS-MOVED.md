# Chunk `ziee-test-harness` — TESTS-MOVED

The harness IS the test infrastructure; it has no `#[cfg(test)]` unit tests of
its own. Its correctness is proven by the entire ziee integration suite running
UNCHANGED through it (the equivalence gate) — no test file is edited, moved, or
ported. The shim preserves every symbol name/signature the suite references.

- **T-harness-server** [stays→ziee] file: `src-app/server/tests/**` covers: `TestServer`/`TestServerOptions`/`start*`/`test_helpers` (~272 files, 1848 hits) — compile + run UNCHANGED via the shim.
- **T-harness-desktop** [stays→ziee] file: `src-app/desktop/tauri/tests/**` covers: `TestServer::start_desktop` (16 files) — compile + run UNCHANGED via the `#[path]` shim + the new dev-dep.

Equivalence evidence (representative end-to-end subset through the extracted
harness): `cargo test --test integration_tests auth::admin_providers hub::migration`
— see BOUNDARY.md `ziee-suite`.
