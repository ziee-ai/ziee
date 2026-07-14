# Chunk BG-3 — BOUNDARY (exit checklist)

## Gate results

| Gate | Result |
|---|---|
| `cargo check -p ziee` | **exit 0** |
| `cargo check -p ziee-desktop` | **exit 0** (adds the `ziee-desktop-harness` dep) |
| `cd sdk && cargo check --workspace` | **exit 0** (all crates + skeleton-server + harness; `ziee-framework` still **build-DB-free**) |
| E8 golden — `types.ui.ts` | **BYTE-IDENTICAL** vs `.extraction/baseline` |
| E8 golden — `types.desktop.ts` | **BYTE-IDENTICAL** |
| E8 golden — `openapi.ui.json` | **CANONICALLY-EQUAL** (`jq -S`) |
| E8 golden — `openapi.desktop.json` | **CANONICALLY-EQUAL** |
| Regenerated openapi/types | restored via `git checkout` |
| Moved unit tests — `ziee-framework embedded_pg::` | **2 passed, 0 failed** |
| Harness names NO code `ziee::` | **confirmed** (only `//!` doc-comment prose) |
| DRIFT | Unresolved drifts: 0 |
| FIX round 1 | New confirmed findings: 0 |

## Build-DB-free proof (ziee-framework invariant preserved)

```
$ grep -nE 'query!|query_as!|query_scalar!' sdk/crates/ziee-framework/src/embedded_pg.rs
$ echo $?
1        # no compile-time query macros — embedded_pg uses runtime sqlx::query + migrator.run
$ cd sdk && cargo check --workspace   # no DATABASE_URL / no build DB
   Finished ... exit 0
```

`ziee-framework` gains `embedded_pg` (postgresql_embedded lifecycle + runtime
`sqlx::query("SELECT 1")` + `Migrator::run`) — none requires a build DB. The
invariant "ziee-framework MUST stay build-DB-free" holds.

## D-full unblocked — proof

```
$ grep -rn 'ziee::' sdk/desktop/harness/src | grep -vE ':[0-9]+://'
(no output — every 'ziee::' in harness/src is a //! doc-comment; zero CODE refs)

$ grep -i path sdk/desktop/harness/Cargo.toml
ziee-core = { path = "../../crates/ziee-core" }
ziee-identity = { path = "../../crates/ziee-identity" }
ziee-auth = { path = "../../crates/ziee-auth" }
# (never the `ziee` app crate)
```

- ziee-desktop provides the concrete `ServerBoot` impl **`ZieeServerBoot`**
  (`server_boot.rs`), returning the harness `BootHandle{addr,pool,jwt}`.
- The desktop live boot (`start_backend_server`) now DRIVES the embedded server
  through that seam; the `auto_login` command + owner read/mint are threaded off
  `BootHandle.{pool,jwt}` (no `ziee::Repos` read remains on the paths D-full
  relocates).
- The embedded-Postgres lifecycle is relocated into `ziee-framework::embedded_pg`
  (generic; both apps' `ServerBoot` impls reuse it).

The three couplings the STOP_REPORT named — (1) the `ziee::Repos` global, (2) the
JWT `OnceLock`, (3) the config statics — are resolved for the desktop-consumer
boot surface: pool + jwt now flow through `BootHandle`; the app-domain
owner-create + `BACKEND_CONFIG`/`BACKEND_STATE` stay app-side (per Chunk D / BA).
So the live Tauri shell (`run`/`run_headless`, the 2 IPC commands,
`create_main_window`) can MOVE into `ziee-desktop-harness` generic over
`Arc<dyn ServerBoot>` — **D-full is unblocked**.

## Wire-safety note (why the golden is byte-identical)

`embedded_pg` + `server_boot` expose no aide/schemars handler or DTO; the two new
`ziee` re-exports (`AppRepository`/`UserRepository`) carry no `JsonSchema` impl.
The moves therefore relocate zero schemars idents → both surfaces' `types.ts` are
byte-identical and `openapi.json` canonically-equal. Had any moved type entered
the wire and drifted `types.ts`, the task's STOP rule would have applied — it did
not.

## Committed
Per the task, the SDK submodule side (`crates/ziee-framework/src/embedded_pg.rs` +
`lib.rs` + `Cargo.toml` + `Cargo.lock` + the standalone `.cargo/config.toml`) is
committed in `sdk/` only — NOT pushed. The ziee side (the shim + re-exports +
desktop `ServerBoot` impl + threading + `src-app/Cargo.lock`) is left uncommitted
for the orchestrator, matching the BG/BG-2 convention. No remote pushed;
`/data/pbya/ziee/ziee` untouched.
