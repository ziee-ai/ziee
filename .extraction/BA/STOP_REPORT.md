# Chunk BA — STOP-and-report (schema-bound core blocked on N6 de-globalization)

The spike is golden-clean, so the chunk **proceeded past the fail-safe gate**.
It then **STOPPED before the full module move + migration relocation** — a
genuine unsafe point, not a golden drift. This is the task's sanctioned
"STOP-and-report rather than force a broken change" outcome.

## The blocker (evidence-backed)

Chunk BA's schema-bound core — moving the auth **repos + `query!` macros** into
`ziee-auth` behind an **auth-only build DB** + relocating the auth migrations —
requires the auth code to compile inside an SDK crate that may name only
`ziee-core` / `ziee-framework` / `ziee-identity`. It cannot, because the auth
module (incl. its repo layer) is hard-wired to **unextracted app-global
singletons**:

| App-global (still ziee-only) | Named by (auth files) |
|---|---|
| `crate::core::Repos` (the global repository aggregator) | `refresh_tokens.rs:8,20,63`, `session_settings.rs:22,153,190` |
| `crate::modules::sync::{publish,Audience,SyncEntity,…}` | `session_settings.rs:24` |
| `crate::core::secrets::storage_key` | `providers/repository.rs:91` |
| `crate::core::AppEvent` / `core::events::EventBus` | `handlers.rs`, `events.rs`, `jwt.rs`, `providers/{events,local,apple,oauth2}.rs` |
| `crate::utils::url_validator::*` (SSRF) | `providers/{health,local,apple,oauth2}.rs` |
| `crate::core::config::JwtConfig` | `jwt.rs` |

Critically, the coupling reaches the **repo layer**, not just handlers:
`refresh_tokens.rs` and `session_settings.rs` call `Repos.session_settings.get()`
/ `Repos.pool()` directly, so even a repo-only sub-move drags the whole `Repos`
aggregator. The wire **DTOs** are entangled too: `AuthResponse` embeds
`TokenPair` (`jwt.rs`) which pulls `JwtConfig`/`secrets`/`EventBus`. The ONLY
subgraph with no app-global edge is `User` + `Group` — exactly the spike.

This is precisely **decision N6**: "a dedicated Chunk BG (Repos/JWT/config behind
traits) BEFORE B3, since the globals gate the whole extraction." B3 delivered
only the **permission-resolution** seam (`IdentityResolver`). `Repos`, the
`AppEvent`/`EventBus` event system, `sync` publish, `secrets`, and `url_validator`
were **never de-globalized**. Until they are (an N6/BG chunk that puts
`Repos`/event-sink/secret-key/SSRF-policy behind injected SDK traits), the
8177-line auth module + its `query!` macros + auth-only build DB cannot move
without either breaking the build or dragging half the app runtime into the SDK.

## What was NOT done (and why it's unsafe now, not merely unfinished)
1. **Full auth-module move** — blocked as above.
2. **Migration relocation + merged migrator + auth-only build DB** — deliberately
   NOT attempted. Relocating the auth migrations out of `migrations/` while the
   auth **code** (its `query!` against `users`/`groups`/`refresh_tokens`/…) stays
   in ziee inverts the dependency: `ziee-auth` would own the schema but ziee would
   own every query that uses it, and the auth-only build DB (whose sole purpose is
   verifying `ziee-auth`'s `query!` macros — §7.1) would verify nothing. It also
   adds real risk to **existing deployments** (rewiring the runtime `migrate!` at
   `core/database/mod.rs:318` + `build.rs:165`) for zero functional gain. The
   correct sequencing is N6 → move auth code → relocate migrations *with* the code.

## Correct next step
Land the N6/BG de-globalization chunk (inject `Repos`, an event sink for
`AppEvent`, `secrets::storage_key`, `url_validator` policy, `JwtConfig`), THEN
re-run Chunk BA: move auth repos+handlers, stand up the auth-only build DB, and
relocate migrations via the N3 build-time directory composition — all now
un-blocked. The golden-spike result in this dir de-risks that future move: the
wire schemas are provably stable under the crate boundary.

## ORDER not appended
`.extraction/ORDER` is intentionally **left unchanged**. BA is not a
gate-complete chunk (no `EA` merged-migrator, no full move), so appending it
would make `extraction-check --all` treat an incomplete chunk as green. The
golden-verified `User`/`Group` move is committed to the SDK submodule as a real
increment the future BA builds on.
