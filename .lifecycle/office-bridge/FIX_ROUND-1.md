# FIX_ROUND-1 — office-bridge (consolidated)

The consolidated blind audit (12 parallel angle-groups over the whole branch diff
vs `origin/main`, `LEDGER.jsonl`) surfaced 8 findings in the relocated code. Fixes
(commit `harden(office_bridge): fix … findings from the consolidated-lifecycle re-audit`):

- **macos.rs / windows.rs cert-staging TOCTOU (MED, security)** — both staged the CA
  at a predictable `temp_dir()/ziee-bridge-cert-<pid>.cer` via non-exclusive `fs::write`
  before a privileged `security add-trusted-cert` / `certutil -addstore Root`. Fixed via
  a shared `platform::stage_cert_der` that creates a private, exclusively-created temp
  dir (`create_dir` fails-if-exists; unix `0700`/`0600`; cert file `create_new`/O_EXCL) —
  closes CWE-377 on both platforms.
- **unsupported.rs dead test (MED, tests-quality)** — `test11_mac_transport_verified` was
  `cfg(target_os=macos)` inside a `cfg(not(macos))` module → never compiled anywhere.
  Removed (real coverage lives in `macos.rs::tests::mac_transport_verified`).
- **migrations 006/007 stale cross-refs (LOW ×2)** — corrected leftover server-crate
  "migration 133" references to the desktop-crate numbers.
- **pane_rpc_test real-LLM gating (LOW, tests-quality)** — REJECTED the finding's
  `#[ignore]` recommendation (A3 forbids diff-added `#[ignore]`); the existing runtime
  soft-skip is the lifecycle-compliant gate. Reworded the doc to state this.
- **settings_mcp_test substring secret-scan (LOW, tests-quality)** — REJECTED: the
  adjacent explicit field-allowlist assertion is the real no-leak guarantee; the
  substring checks are harmless defense-in-depth. (LEDGER status: rejected.)

The re-audit of the fixes (phase-7 round) then found **1 new** confirmed finding.

**New confirmed findings:** 1
