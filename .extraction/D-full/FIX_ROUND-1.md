# Chunk D-full — FIX round 1

Blind multi-angle audit (LEDGER.jsonl — 17 findings across 13 angles incl.
`equivalence`, two `security`, `ordering`, `error-handling`, `concurrency`,
`boundary`, `app-agnostic`, `platform-gating`, `wire`, `feature-unification`,
`build`, `cleanup-safety`, `api-surface`, `tests`) reconciled against every diff
hunk (AUDIT_COVERAGE.tsv — 15 hunks, ≥3 angles each).

## Findings surfaced + resolved DURING the drift-convergence loop

All were resolved before the gate; none deferred; each re-verified.

- **Seam value → `Arc<dyn ServerBoot>`** (equivalence/types): the harness helper
  must own the seam for the spawned task, but BG-3's `start_backend_server` held
  a concrete `ZieeServerBoot` value called via `ServerBoot::boot(&boot)`. Fixed by
  coercing to `let boot: Arc<dyn ServerBoot> = Arc::new(ZieeServerBoot::new(...))`
  and taking `Arc<dyn ServerBoot>` in `spawn_boot_then_window`. Same dispatch to
  the same impl. Re-verified: `cargo check -p ziee-desktop` exit 0.

- **`on_ready` Send/'static bounds** (concurrency): the six post-boot steps
  (incl. `state.set_ready(true)`) move into an app closure the harness awaits
  inside `tauri::async_runtime::spawn`. Confirmed the closure + its future satisfy
  `FnOnce(BootHandle) -> Fut + Send + 'static, Fut: Future + Send + 'static`
  (BackendState is `Arc<Mutex>` Send+Sync; the ziee ops are all Send). Compiles
  clean — the bound is exactly what the original single spawn already required.

- **`tauri` as a fresh harness dep** (build/feature-unification): adding `tauri`
  to the standalone SDK workspace could (a) fail to build a lib without
  `tauri-build`, or (b) duplicate/mis-unify against ziee-desktop's tauri. Verified
  (a) `tauri` builds as a plain lib (no `generate_context!` in the harness) — `cd
  sdk && cargo check --workspace` exit 0; (b) versions match ziee-desktop (2.8 /
  decorum 1.1.1) so the single `src-app/Cargo.lock` unifies onto one tauri build
  (2-line lock delta, no dup) — `cargo check -p ziee-desktop` exit 0.

- **Success-log placement** (ordering): moving `info!("Backend server started
  successfully on {}", …)` into the harness could drift its position relative to
  the jwt/pool stash. Verified it fires immediately after `Ok(handle)` and BEFORE
  the app's `on_ready` (which does the stash) — identical to BG-3. Not a drift.

- **No re-export shim for `create_main_window`** (api-surface): confirmed it was
  a private module-internal `fn` with no external importer (grep), so removing it
  from `mod.rs` breaks no path — no shim owed.

## Re-audit verdict

Every AUDIT_COVERAGE hunk carries ≥3 angles and reconciles to a LEDGER
`equivalent` / `preserved` / `additive` / `unified` / `contract-held` /
`unchanged` verdict. No `severity` above `info`. E8 golden byte-identical on both
surfaces; harness `ziee::` code-ref count 0; three cargo checks + the 7 backend
unit tests green.

**New confirmed findings:** 0
