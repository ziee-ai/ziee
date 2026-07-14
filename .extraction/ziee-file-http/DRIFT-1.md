# Chunk `ziee-file-http` — DRIFT-1

Drift count: **0**.

The implementation matched CUT.md/TRANSFORMS.md as authored — no plan-vs-code
divergence surfaced during the drift pass. Two compile-time facts were confirmed
(not drift, but recorded):

1. `axum::extract::Query`'s `OperationInput` is gated behind aide's `axum-query`
   feature, which no SDK crate previously enabled (the server links a
   workspace-level aide that already has it). Declared `axum-query` on
   ziee-file's optional aide so it compiles STANDALONE; the full-build feature
   union is unchanged. (Planned as a routes dep; the exact feature flag was
   discovered at first compile.)

2. `main.rs` re-declares its own module tree and builds its own router (separate
   from `lib.rs::setup_server`), so the `build_file_context` Extension layer had
   to be added at BOTH sites. Caught by the bin's `build_file_context is never
   used` dead-code warning after the lib-only edit; adding the `main.rs` layer
   cleared it. This is exactly the two-site pattern the prior chunks used for
   `build_auth_context`.

Both were resolved in the same implementation pass (T-1 aide feature, T-8 dual
layer); neither changed the plan's design.
