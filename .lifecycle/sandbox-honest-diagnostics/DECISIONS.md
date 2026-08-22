# DECISIONS — sandbox honest diagnostics

### DEC-1: What log level for the routine child-exited-early (EPIPE) seccomp-pipe write?
**Resolution:** `debug`. The call already fails correctly at bwrap spawn (a
partial filter is rejected by libseccomp, never loaded), bwrap reports its own
error, and 175/204 of these are immediately followed by an MCP upstream failure —
so the event is non-actionable, expected, and high-frequency (~204/day). `debug`
is off by default, eliminating the alert fatigue while remaining available for
diagnosis. Not `warn`/`info` — those stay in default output and reproduce the
fatigue.
**Basis:** convention — the crate already logs expected-and-non-actionable
lifecycle events at `debug`/`info` and reserves `error!` for genuine failures
(e.g. `BWRAP_SPAWN_FAILED`); the loud `error!` is retained for the genuine
truncation case only.

### DEC-2: What log level stays on the GENUINE truncated write (non-EPIPE)?
**Resolution:** `error!`, with the EXISTING loud wording preserved verbatim
("seccomp BPF write was truncated … WILL FAIL — hardening claim 'seccomp: on'
was not delivered"). This is the trap the task calls out: quieting the EPIPE
noise must not silence a real hardening-not-delivered event.
**Basis:** codebase — the current message + level is correct for the genuine case;
only the EPIPE case was mis-filed under it.

### DEC-3: How wide is the Defect-1 message fix — the one reported site, or all `SANDBOX_NOT_INITIALIZED` sites?
**Resolution:** all of them (SDK `execute.rs`, ziee `handlers.rs` ×2,
`version_handlers.rs` ×3, `tools/files.rs`). They are the SAME misleading-message
class; each asserts a guessed cause ("module disabled or not yet booted",
"enabled: false in config or boot probe failed", or a bare "not initialized").
Routing them all through `init_status().explain()` is the surgical message-only
fix — it adds no behavior, and leaving 5 of 6 misleading would be inconsistent
and re-flaggable.
**Basis:** convention — one honest source of truth for the reason
(`SandboxAvailability::explain()`) consumed at every site.

### DEC-4: Which host roots does redaction scrub, and with what placeholders?
**Resolution:** `state.workspace_root` → `<sandbox-workspace>` and the resolved
rootfs `ensure.mount_dir` → `<sandbox-rootfs>`. These are exactly the two host
absolute-path families bwrap binds (`--bind <workspace>` and `--ro-bind
<mount_dir>/usr` etc.), so they are the only host paths bwrap's setup diagnostics
can emit. A literal-prefix `str::replace` is safe: the workload inside the sandbox
sees `/home/sandboxuser` and `/usr`, never the host paths, so redaction is a
no-op on all legitimate output and fires only on bwrap's own diagnostics.
**Basis:** codebase — `build_hardening_prefix` (sandbox.rs) binds precisely these
two roots; both are already in scope in `execute_command_with_mounts`.

### DEC-5: Is any operational tunable introduced (→ admin-configurable settings row)?
**Resolution:** No. The only "knobs" are two log LEVELS (fixed `debug`/`error`)
and the redaction behavior (always-on). Log verbosity is already governed by the
process-wide `tracing`/`RUST_LOG` filter, not a per-feature setting; the
no-host-path-leak redaction is a security invariant that must not be
operator-disableable. Both are correctly fixed constants, not settings rows.
**Basis:** convention — matches how the module already treats hardening + logging
(no `code_sandbox_settings` field governs log level or the existing 5xx-path
redaction).
