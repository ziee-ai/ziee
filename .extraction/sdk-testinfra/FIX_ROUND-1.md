# Chunk `sdk-testinfra` — FIX round 1

**Fix count: 0.**

The blind multi-angle audit (LEDGER ti-01..ti-12, 12 distinct angles incl.
`equivalence` + `security`) surfaced no HIGH/MEDIUM finding requiring a code
change. Every angle resolved `verified` on first pass:

- byte-parity of the 4 moved generators (write-mode diff) — PASS
- cross-workspace seed-registry (`ui` + `desktop --src src`) — PASS
- cross-session container-reap safety (shared {pid,runId} lock) — preserved
- clean build (test-e2e + gallery tsc, ui + desktop tsc, node --check, 17 unit tests) — PASS
- backward-compat (ziee e2e untouched; repointed scripts behave identically) — PASS
- no generated/Rust/OpenAPI impact — PASS
- vite plugin repoint boot smoke — PASS

The one judgment call — `gen-testid-registry` producing 1588 vs the committed
kit file's 1590 — was NOT fixed by hacking the walk (which would risk a wrong
result + entangle the kit migration); it was correctly DEFERRED (T-9/D-4). That
is a scope decision, not a fix.

No fixes needed → converged.
