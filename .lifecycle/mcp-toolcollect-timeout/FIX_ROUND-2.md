# FIX_ROUND-2 — mcp-toolcollect-timeout

Re-audit of the round-1 FIX diff (commit `ddb7cd28a`, `git diff 8eb01cb48..HEAD`)
with two fresh blind angles differing in kind: correctness + concurrency-resource.

## Corroboration outcome

- **concurrency-resource**: no findings. Verified lock ordering, guard scope,
  cancellation validity, and the always-mode `break` path.
- **correctness**: confirmed the round-1 fixes are sound — no double-record (the
  `Ok(Err)` connect-error arm and the `Err(_elapsed)` timeout arm are mutually
  exclusive; the always-mode timeout arm `continue`s so it can't also hit the
  build-error arm), `record_failure_into` is byte-identical to the former inline
  body, the `break` skips no cleanup, server_id args are correct, and the rewritten
  breaker test drives real code (not tautological). It raised 4 LOW findings, all
  single-angle (none corroborated ≥2, none security/data-loss/authz) → none are
  promotable to mandatory work under the corroboration rule.

## Round-2 findings — disposition

- **F-r2-4 (LOW) — FIXED anyway** (wired-but-not-behaving, and it touches a stated
  invariant): always-mode recorded into the breaker but never *consulted* it, so the
  opened breaker did not suppress always-mode's own re-dials — INV-3's re-dial
  suppression was not actually realized for always-mode. Fix: always-mode now calls
  `check_connection_breaker(server.id)` before building the session and skips
  (warn + continue) while the breaker is open. `check_connection_breaker` made
  `pub(crate)`. This is what makes INV-3 genuinely hold for always-mode.
- **F-r2-1 / F-r2-2 / F-r2-3 (LOW) — WONTFIX.** The timeout budget wraps the whole
  connect operation, which includes pre-dial DB/setup work; a DB/setup slowness that
  trips the timeout over-records a "connection" failure. Bounded and self-healing
  (base cooldown 1s), inherent to wrapping the whole operation in one timeout (the
  auto path's own `get_or_create` has the same shape), and F-r2-3 matches the auto
  path's `create_session_tracked`, which likewise records ANY `McpSession::new`
  error. Distinguishing connect-only slowness would require restructuring
  `get_or_create` — out of scope for this hardening fix.

## New confirmed findings: 0

No promotable (≥2-angle / oracle / security-class) findings in round 2. Profile:
round 1 = 10 confirmed (2 corroborated HIGH), round 2 = 0 promotable (4 LOW,
uncorroborated). Detection profile is decaying; no round concentrated on a guard
file (no GUARD-SUB). **Converged** — the fix/re-audit loop terminates.
