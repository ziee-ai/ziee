# MIGRATE-squash — DRIFT-1

Drift-convergence pass over the reconstruction (implement → gate → converge).

- **DRIFT-1.1** — verdict: resolved. pg_dump VARCHAR CHECK non-idempotency (PG18)
  caused 54 fingerprint lines to differ on the first squash pass. Fixed at source
  by rewriting the 27 expanded CHECKs to the original `IN(...)` fixed-point (T-3).
  Re-check: schema.fp squash-vs-baseline IDENTICAL.
- **DRIFT-1.2** — verdict: none. Feared FK-ordering drift from module-split — the
  deferred-FK band makes cross-module table order irrelevant; fresh-DB apply is
  clean.
- **DRIFT-1.3** — verdict: none. Feared auth-only build DB breakage (shared
  trigger fn) — auth redefines it idempotently; auth-only standalone apply + sdk
  workspace check both clean.

**Unresolved drifts:** 0
