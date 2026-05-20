//! Tier 4 — seccomp filter behavior under bwrap.
//!
//! Skipped if libseccomp / cargo feature `code_sandbox_seccomp` is not
//! linked, OR if no rootfs is mounted.

#[test]
#[ignore]
fn seccomp_filter_is_documented_by_planner() {
    // Real assertion (the Seccomp:2 / Seccomp_filters:1 evidence) lives
    // in src/modules/code_sandbox/probes.rs unit tests once the
    // libseccomp Rust binding is wired in. This file is a placeholder
    // to keep the test taxonomy in sync with the plan.
    //
    // To run the live filter assertion locally:
    //   1. `apt install libseccomp-dev`
    //   2. `cargo test --features code_sandbox_seccomp -- --ignored seccomp`
    //   3. Or invoke bwrap directly with `--seccomp <fd>` against a
    //      filter exported via `scmp_export_bpf`.
    eprintln!("test skipped: live seccomp assertion not yet wired");
}
