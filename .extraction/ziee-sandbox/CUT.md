# Chunk `ziee-sandbox` — the full sandbox subsystem extraction (5-crate scope)

Scope per the human decision: extract the FULL sandbox subsystem into the SDK —
the **4 support crates** (clean leaf MOVES) **+ the `ziee-sandbox` engine**
(abstract over rootfs + resource-limits via injected seams). Re-implemented on the
CURRENT base (branch `feat/sdk-sbxsupport`), superseding the parked
`sdk-sandbox-wt` design which was Increment-1-scaffold-only.

Two phases. **Phase 1 (DONE + verified green this session): the 4 support crates.**
**Phase 2 (STOP-and-report): the `ziee-sandbox` engine** — the parked Increment-2
seam spec is under-specified for the CURRENT base; the engine boundary is messier
than it assumed (see `BOUNDARY.md`). Executing it correctly requires an EXTENDED
seam + moving ~250 KB of VM-backend code that is `#[cfg]`-out on Linux (thus
un-verifiable here) and under-specified. Per the task's explicit STOP criterion
("engine boundary messier than the parked design assumed") + the "don't fake a
pass / don't weaken isolation" mandate on this security-critical, byte-identical
hardening move, Phase 2 is reported for greenlight rather than executed blind.

---

## PHASE 1 — the 4 support crates (EXECUTED, byte-identical MOVE)

`src-app/{sandbox-guest-agent, sandbox-vm-launcher, sandbox-vm-protocol,
sandbox-seccomp}` → `sdk/crates/` (names kept; domain-agnostic leaves).

- **Crate bodies byte-identical** — pure relocation (`git mv`), no source edit.
  Their internal path deps (`../sandbox-vm-protocol`, `../sandbox-seccomp`) stay
  valid because all four move together as siblings under `sdk/crates/`.
- **`[lints] workspace = true`** in all four is kept byte-identical by adding the
  matching `[workspace.lints]` to `sdk/Cargo.toml` (mirrors the src-app policy:
  `unused_imports`/`unused_mut` deny, `dead_code` warn,
  `clippy::too_many_arguments` allow). SDK crates that don't opt in are unaffected.
- **SDK workspace** picks them up via its `members = ["crates/*"]` glob.
- **src-app workspace** members list drops the four (replaced by a comment).
- **`src-app/server/Cargo.toml`** repoints the two path deps that the server
  actually consumes:
  - `sandbox-seccomp` (optional, `code_sandbox_seccomp` feature, `cfg(linux)`)
    → `../../sdk/crates/sandbox-seccomp`
  - `sandbox-vm-protocol` (`cfg(any(linux,macos,windows))`)
    → `../../sdk/crates/sandbox-vm-protocol`
- **No source edits**: the Rust crate names (`sandbox_vm_protocol`,
  `sandbox_seccomp`) are unchanged by relocation, so every
  `use sandbox_vm_protocol::…` / `sandbox_seccomp::…` in `code_sandbox` (backends
  `vm_client`/`mac_vm`/`wsl2`/`vm_long_lived`, `sandbox.rs`, `probes.rs`,
  `resource_limits*.rs`) resolves unchanged. The desktop crate + guest-agent reach
  them via the same cross-workspace path deps.

### Phase-1 gates (ALL green)
- `cargo check -p ziee` = **0** (only 3 pre-existing warnings, unrelated).
- `cargo check -p ziee-desktop` = **0**.
- `cd sdk && cargo check --workspace` = **0** (stock; the four crates + all
  existing SDK crates check). `sandbox-seccomp`/`sandbox-vm-launcher` build to
  their platform stubs on Linux (Linux-only libseccomp / macOS-only libkrun) as
  before.

---

## PHASE 2 — `ziee-sandbox` engine (abstract) — STOP-and-report

The parked design's abstraction (two injected seams — `RootfsProvider` +
`ResourceLimitsProvider` — into a de-`pool`-ed `CodeSandboxState`, with
`build_bwrap_argv`/cgroup/prlimit/seccomp moved byte-for-byte) is architecturally
sound and REUSABLE. But the CURRENT base has coupling the parked
14-substitution Increment-2 spec did NOT enumerate. Full evidence in `BOUNDARY.md`;
summary:

1. **Reverse-dep INSIDE the argv builder.** `sandbox.rs`'s
   `build_command`/`build_bwrap_argv` region reaches
   `crate::modules::lit_search::fulltext::cache::{conversation_view_dir,
   SANDBOX_MOUNT_PATH}` to add the `/lit` `--ro-bind`. The single most
   security-sensitive function (must move byte-for-byte) references a ziee app
   module → cannot move to a build-DB-free SDK crate as-is. Fixable by lifting the
   `/lit` bind to a caller-injected `extra_ro_bind` (argv output identical), but
   that is a real seam addition, not a transcription.
2. **`tools/execute.rs` → `handlers::apply_workspace_mode`** — the moved execute
   path calls a helper in `handlers.rs` (STAYS). The fn is a pure fs chmod
   (DB-free) → relocatable, but omitted from the parked spec.
3. **`mac_vm.rs` reads `runtime_mount::READY` directly** (the per-flavor caps
   OnceCell global), not only via `ensure_rootfs_ready`. Needs a provider access
   path the seam doesn't define.
4. **VM backends → embedded-agent staging.** `mac_vm`/`wsl2` call
   `embedded::ensure` / `wsl2_agent_embedded::ensure` / `helper_service::client`
   (stage the in-guest agent binary; use `crate::core::get_app_data_dir`). This is
   a whole third concern (guest-agent binary provisioning) the parked
   two-seam design never covered — it needs either a 3rd seam or `embedded.rs` +
   `wsl2_agent_embedded.rs` moved WITH `app_data_dir` threaded through config.
5. **`streaming.rs` (SSE fetch-progress)** uses `runtime_mount::is_flavor_cached`,
   `runtime_fetch::is_fetch_in_flight`, and `runtime_fetch::ensure_fetched` with a
   REAL progress callback — none in the parked seam. Reclassify as STAY (app UI
   plumbing), which deviates from the parked CUT (which listed it as moved).

Items 3-4 live on the `mac_vm`/`wsl2` arms, which are `#[cfg(target_os =
"macos"/"windows")]` → **not compiled on this Linux host**, so a wrong move would
NOT trip the Linux `cargo check` gate but WOULD silently break the mac/windows
builds (verified only on those hosts). Moving ~250 KB of under-specified,
un-verifiable VM-backend code and asserting a pass violates the task's
"don't fake a pass" rule.

### What the extraction NEEDS (revised seam, for greenlight)
- The parked `RootfsProvider` + `ResourceLimitsProvider` + vocabulary types +
  de-`pool`-ed `CodeSandboxState` + `caps`/`limits` moves — all still correct.
- **PLUS**: (a) lift the `/lit` bind out of the argv builder into a
  caller-supplied extra-bind (keep argv byte-identical); (b) relocate the pure
  `apply_workspace_mode`; (c) a `RootfsProvider` method (or state accessor) for
  the `READY` caps snapshot mac_vm reads; (d) a guest-agent-staging decision (3rd
  seam vs. move-embedded-with-config, threading `app_data_dir`); (e) reclassify
  `streaming.rs` as STAY.
- Then execute the byte-for-byte hardening move + the two provider impls in ziee +
  the re-export shims + the `version_manager` in-mem/DB split, and run the
  hardening `assert_eq!` audit on the real before→after `build_bwrap_argv`.

### Phase-2 gates (NOT yet run — pending greenlight of the revised seam)
- `-p ziee`/`-p ziee-desktop`/`sdk --workspace` = 0, build-DB-free grep empty,
  golden byte-identical both surfaces, tier-1 sandbox unit tests move+pass,
  hardening `build_bwrap_argv`/cgroup/seccomp byte-diff empty.

## Cross-platform caveat (carries into Phase 2 when executed)
Only the Linux `linux_bwrap` path is runtime-verifiable on this host. The
`mac_vm`/`wsl2` arms are `#[cfg]`-out on Linux; their compile-gating must stay
correct and their runtime verification is a mac/windows-host follow-up (the
project's cross-platform verify loop). This is precisely why moving them blind in
this session is deferred.
