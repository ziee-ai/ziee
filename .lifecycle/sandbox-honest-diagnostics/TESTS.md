# TESTS — sandbox honest diagnostics

All tiers are backend unit tests (in-source `#[cfg(test)]`) — the fix surface is
pure Rust logic (message text, log-outcome classification, string redaction). No
frontend path is touched, so no `tier: e2e` is required. No permission is
introduced, so no `[negative-perm]` spec is required.

Each defect's assertion is about MESSAGE CONTENT / LOG OUTCOME / REDACTION, so
the tests assert those directly (not exit codes).

- **TEST-1** (tier: unit) [acceptance] [invariant: INV-1] [covers: ITEM-1] file: `sdk/crates/ziee-sandbox/src/config.rs` — asserts: `SandboxAvailability::explain()` returns an accurate, specific reason for EVERY variant, and in particular the `HostUnsupported` reason mentions bwrap/host and NO variant's reason contains the false phrase "not yet booted".
- **TEST-2** (tier: unit) [acceptance] [invariant: INV-1] [covers: ITEM-2, ITEM-3] file: `sdk/crates/ziee-sandbox/src/tools/execute.rs` — asserts: `execute_command` on an uninitialized sandbox returns a `SANDBOX_NOT_INITIALIZED` error whose message CONTAINS the recorded `init_status().explain()` reason and does NOT contain "module disabled or not yet booted".
- **TEST-3** (tier: unit) [acceptance] [invariant: INV-2] [covers: ITEM-4] file: `sdk/crates/ziee-sandbox/src/sandbox.rs` — asserts: the write-outcome classifier maps `offset<total & EPIPE` → `ChildGone`, `offset<total & other errno / EOF` → `Truncated`, `offset==total` → `Complete`; and (the loud-stays-loud trap) that a genuine truncation is NOT classified as ChildGone.
- **TEST-4** (tier: unit) [acceptance] [invariant: INV-3] [covers: ITEM-5] file: `sdk/crates/ziee-sandbox/src/tools/execute.rs` — asserts: `redact_host_paths` replaces the host workspace root and rootfs mount-dir prefixes with sandbox-relative placeholders, and is a no-op on text containing no host path.
- **TEST-5** (tier: unit) [acceptance] [invariant: INV-3] [covers: ITEM-5] file: `sdk/crates/ziee-sandbox/src/tools/execute.rs` — asserts: a simulated bwrap dead-mount stderr line (`bwrap: Can't mount <host rootfs>/usr ...`) run through the redaction helper with real host roots no longer contains the host absolute path (the exact string a workload never emits but bwrap's setup failure does).

## Plan-coverage check

Every ITEM is covered: ITEM-1→TEST-1; ITEM-2→TEST-2; ITEM-3→TEST-2; ITEM-4→TEST-3;
ITEM-5→TEST-4, TEST-5. No item is descoped.
