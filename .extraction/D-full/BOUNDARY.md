# Chunk D-full — BOUNDARY (exit checklist) — FINAL chunk

## Gate results

| Gate | Result |
|---|---|
| `cargo check -p ziee` | **exit 0** (pre-existing warnings only) |
| `cargo check -p ziee-desktop` | **exit 0** (pre-existing warnings only) |
| `cd sdk && cargo check --workspace` | **exit 0** (compiles tauri 2.11.5 + tauri-plugin-decorum + tray-icon + the harness `window` module) |
| E8 golden — `types.ui.ts` | **BYTE-IDENTICAL** vs `.extraction/baseline` |
| E8 golden — `types.desktop.ts` | **BYTE-IDENTICAL** |
| E8 golden — `openapi.ui.json` | **CANONICALLY-EQUAL** (`jq -S`) |
| E8 golden — `openapi.desktop.json` | **CANONICALLY-EQUAL** |
| Regenerated openapi/types | restored via `git checkout` |
| ziee-desktop `modules::backend::` unit tests | **7 passed, 0 failed** |
| Harness names NO code `ziee::` | **confirmed** — `grep -rn 'ziee::' sdk/desktop/harness/src \| grep -v '//'` empty (count 0) |
| DRIFT | Unresolved drifts: 0 |
| FIX round 1 | New confirmed findings: 0 |

## What moved

| Symbol | From | To |
|---|---|---|
| `create_main_window` (private `fn`, verbatim per-OS body) | `ziee-desktop modules/backend/mod.rs` | `sdk/desktop/harness/src/window.rs` (`pub`, `+ &WindowConfig`) |
| boot→window spawn skeleton (inline) | `start_backend_server` | `harness window.rs::spawn_boot_then_window` (generic over `Arc<dyn ServerBoot>`) |
| `WindowConfig` | — (new) | `harness window.rs` |

ziee-desktop's `start_backend_server` is now a thin consumer: build
`Arc<dyn ServerBoot>` (ZieeServerBoot) + a `WindowConfig`, hand the six domain
post-boot steps to `spawn_boot_then_window` as the `on_ready` closure. The
harness owns the spawn, the per-OS window construction, and the
create-window-on-both-paths contract — all generic over the seam.

## What STAYS app-side (the honest app/domain boundary — NOT a STOP)

`run` / `run_headless` / `register_desktop_invoke_handler` + the two IPC commands
(`get_server_port`, `auto_login`) remain in `ziee-desktop`. They wrap the app
module system + `ziee::Config` + `start_server_with_routes` ("the app's entire
server assembly", per `boot.rs`) and resolve in-crate `#[tauri::command]` macros;
moving them would force the harness to name `ziee::`, breaking the reusability
contract. This is the same app/domain seam BG-3 recorded — the window/boot
*lifecycle* is what D-full moves, driven through `Arc<dyn ServerBoot>`.

## Wire-safety note (why the golden is byte-identical)

`WindowConfig` / `create_main_window` / `spawn_boot_then_window` expose no
aide/schemars handler or DTO and carry no `JsonSchema` — they never enter the
OpenAPI surface. The move relocates zero schemars idents, so both surfaces'
`types.ts` are byte-identical and `openapi.json` canonically-equal. Had any moved
type entered the wire and drifted, the task's STOP rule would have applied — it
did not.

## Runtime caveat (post-merge E2E boundary, honest — same as BG-3)

The moved window/boot lifecycle is **runtime-only**: it opens a real Tauri window
driven through `ServerBoot::boot()`, which the Bash-tool integration harness
cannot exercise (no display server / live GUI boot). Its behavioural proof is the
desktop launch + window-shows-on-boot-success-AND-failure E2E — the orchestrator's
post-merge desktop-E2E step. Statically proven here: three cargo checks green (all
OS `#[cfg]` arms compile; linux built), the golden byte-identical, the 7 backend
unit tests pass. **Not a STOP** — the seam is threaded and works.

## Committed

- **SDK submodule** (`sdk/`): `window.rs` (new) + `lib.rs` + `Cargo.toml` +
  `Cargo.lock` committed at **`3b3544a4e26bf9484bfd2983f82af39246c56f5b`** — NOT
  pushed.
- **ziee side** (`feat/sdk-extraction`): the thin `start_backend_server` rewrite
  + `create_main_window` removal (`modules/backend/mod.rs`), `src-app/Cargo.lock`,
  the submodule pointer bump, and `.extraction/D-full/` artifacts + `ORDER`
  committed. `src-app/server/vendor/pgvector` left untracked (never staged). No
  remote pushed; `/data/pbya/ziee/ziee` untouched.

## Extraction complete

D-full is the FINAL chunk. `ORDER` now ends with `D-full`. The desktop shell's
window + boot→window lifecycle lives in the reusable `ziee-desktop-harness`,
generic over the `ServerBoot` seam; `ziee-desktop` is a thin consumer providing
`ZieeServerBoot` + its `WindowConfig` + its domain routes/commands.
