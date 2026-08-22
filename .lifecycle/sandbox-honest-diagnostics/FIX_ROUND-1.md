# FIX_ROUND-1

Blind audit (phase 6) ran two distinct-kind angles — **design-conformance** and
**security** — over `main...HEAD`. 8 findings triaged; 3 confirmed as WORK, 5
triaged wontfix (nits / out-of-scope, all recorded in LEDGER.jsonl with
rationale). No `BLOCKED`.

## Confirmed findings fixed this round

- **INV-1 / handlers.rs:124 (design-conformance, med)** — the MCP JSON-RPC
  not-initialized producer emitted the guessed disjunction "code_sandbox.enabled
  = false or boot probes failed" on the MODEL-facing path. Rerouted through
  `config::init_status().explain()` (honest recorded reason). This was the
  highest-value finding: the original fix rerouted every REST `AppError` site but
  missed the two direct model/stream entry points, one of which literally
  reproduced the anti-pattern INV-1 targets.
- **INV-1 / streaming.rs:266 (design-conformance, low)** — the streamed
  not-initialized producer emitted a vague reasonless string. Rerouted through
  `config::init_status().explain()`.
- **INV-3 / execute.rs redaction (design-conformance + security, med,
  corroborated ×2)** — `redact_host_paths` covered only `workspace_root` +
  `mount_dir`, missing the workflow/provider mount sources (`StagedMount.host_path`,
  e.g. a desktop host folder) and the caller ro-binds (`ctx.extra_ro_binds`, e.g.
  the `/lit` view cache path). Extended the helper with an `extra_host_sources`
  slice and now scrub EVERY host bind-source bwrap can name; the inaccurate
  "the two roots here are exactly the absolute-path families bwrap binds" comment
  was corrected. TEST-4/TEST-5 extended to cover a provider mount + a caller
  ro-bind (both the pure helper and a simulated bwrap dead-mount-source stderr).

## Findings triaged wontfix (with rationale in ledger)

- F2 (canonicalized/symlink form + /proc/mountinfo readability) — the target
  vector (bwrap setup stderr) echoes the configured argv strings verbatim, so it
  is covered; the `/proc` introspection vector is pre-existing and out of scope
  (host paths aren't secrets under the module threat model).
- F3 (over-redaction on a misconfigured short root) — roots are always deep
  resolved absolute paths; legitimate command output never contains them.
- INV-2 EOF-stale-EPIPE nit — unreachable (a pipe write of a non-zero buffer
  never returns 0); the safe default for an unexpected EOF is the loud `Truncated`.
- F5 (deployment-posture disclosure) — no secrets; masked on the MCP tools/call
  path by `map_tool_error`; the deliberate model-facing exposure of the
  non-sensitive reason is the honest-diagnostics goal.

## Termination

Tier is **LIGHT** (small message/log/redaction diff; no new permission,
migration, module, or public API/schema change). Per the LIGHT termination rule
(one blind round complete), the loop ends here. The fix diff is small and
self-reviewed against the same two angles; no new class of finding was
introduced (the extension reuses the tested pure helper; the two message sites
route through the already-unit-tested `explain()`).

**New confirmed findings:** 0
