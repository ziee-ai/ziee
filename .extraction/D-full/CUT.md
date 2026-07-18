# Chunk D-full — move the live desktop window/boot shell into `ziee-desktop-harness` (CUT manifest)

D-full is the **FINAL** chunk. It moves the genuinely-app-agnostic *window +
boot→window orchestration* shell out of ziee-desktop into the reusable harness,
generic over the BG-3 `ServerBoot` seam, leaving ziee-desktop a thin consumer.
Equivalence-preserving MOVE (E8 golden byte-identical on BOTH surfaces —
machine-verified, then restored).

## The move (one relocation, one generalization)

1. **`create_main_window`** — moved **verbatim** (per-OS window construction:
   macOS overlay-titlebar + traffic-lights, Windows decorum overlay, Linux WM
   decorations, the Mica/Acrylic/Blur effects) from ziee-desktop's
   `modules/backend/mod.rs` into `sdk/desktop/harness/src/window.rs`. The five
   app-tunable literals (title, inner-size, min-size, effect radius, traffic-
   light inset) become a `WindowConfig` the app passes; the per-OS chrome logic
   is unchanged (a second app gets the identical look).
2. **The boot→window spawn skeleton** of `start_backend_server` — generalized
   into `spawn_boot_then_window(boot, app_handle, window, on_ready)`: the harness
   owns `tauri::async_runtime::spawn` → `ServerBoot::boot()` → success/failure
   match → `create_main_window` on BOTH paths + the two log strings. The app's
   six domain post-boot steps (stash jwt/pool, desktop migrations, host-mount
   provider, ensure-owner, MCP backfill, memory-default, ready-flag) are threaded
   in via the app-supplied `on_ready` closure — **same order, same error
   strings**.

The harness names ONLY the `ServerBoot`/`BootHandle` seam + `tauri` — **zero code
`ziee::`** (machine-verified). `tauri` + `tauri-plugin-decorum` become harness
deps (deferred through Chunk D precisely for this chunk).

## Files — SDK submodule (`sdk/`)

### NEW (1)
- `desktop/harness/src/window.rs` — `WindowConfig`, `create_main_window`
  (verbatim per-OS body, literals → config fields), and
  `spawn_boot_then_window` (the boot→window skeleton generic over
  `Arc<dyn ServerBoot>`, running the app's `on_ready` domain closure between a
  successful boot and window creation).

### MODIFIED (3)
- `desktop/harness/src/lib.rs` — `pub mod window;` + re-exports
  (`WindowConfig`, `create_main_window`, `spawn_boot_then_window`); doc rewrite
  (D-full-present-contents + the kept-app-side rationale, replacing the
  BG-3-prerequisite deferral note).
- `desktop/harness/Cargo.toml` — add `tauri = { version = "2.8", features =
  ["tray-icon"] }` + `tauri-plugin-decorum = "1.1.1"` (version-matched to
  ziee-desktop so the single `src-app/Cargo.lock` unifies onto one tauri build).
- `Cargo.lock` — regenerated (tauri tree pulled into the standalone SDK
  workspace lock).

## Files — ziee app side (`src-app/`)

### MODIFIED (2)
- `desktop/tauri/src/modules/backend/mod.rs` — (a) add
  `use ziee_desktop_harness::window::{spawn_boot_then_window, WindowConfig};`;
  (b) `start_backend_server` now builds `Arc<dyn ServerBoot>` + a `WindowConfig`
  and hands the six post-boot steps to `spawn_boot_then_window` as the `on_ready`
  closure (byte-identical step order + log strings); (c) the ~90-line
  `create_main_window` fn is **removed** (moved to the harness) and replaced by a
  provenance comment. It was a private `fn` with no external importer → **no
  re-export shim needed**.
- `Cargo.lock` — regenerated (harness gains the tauri edge; already an edge of
  ziee-desktop, so the lock delta is minimal).

## What STAYS app-side (fundamentally app-domain — not moved)

- **`run` / `run_headless`** — wrap the app's whole module system
  (`core::create_desktop_modules` / `initialize_modules` /
  `build_desktop_api_routes`) + `ziee::Config` assembly. `run_headless` calls
  `ziee::start_server_with_routes` inline (it is the test path and does not even
  use the seam) and is saturated with `ziee::` + `crate::modules::`.
  `start_server_with_routes` is "the app's entire server assembly" (per
  `boot.rs`) and cannot move without the harness naming `ziee::`.
- **`register_desktop_invoke_handler` + the two IPC commands**
  (`get_server_port`, `auto_login`) — the `#[tauri::command]` macros generate
  `__cmd__*` idents that only resolve in-crate (per the existing doc), and
  `auto_login` reaches `ziee::UserRepository` / `ziee::refresh_tokens`
  (app-domain). Kept app-side per the task's "keep tauri command registration
  app-side if domain-specific" rule.
- **The `.on_window_event` close-cleanup** (inside `run`) — mixes a domain step
  (`remote_access::tunnel_driver().stop()`) with `ziee::cleanup_server()`; lives
  inside the app-side Tauri builder chain, not a movable unit.
- **`BACKEND_CONFIG` / `BACKEND_STATE` / `JWT_SERVICE` / `SERVER_POOL`** statics,
  `create_desktop_config`, `ensure_persistent_storage_key`, CORS allowlist +
  feature overrides + branding — app Tauri plumbing (per Chunk D + BG-3).

## Symbols

| Symbol | From | To |
|---|---|---|
| `create_main_window` (private `fn`) | `ziee-desktop modules/backend/mod.rs` | `harness window.rs` (`pub`, `+ &WindowConfig` arg) |
| boot→window spawn skeleton (inline) | `start_backend_server` | `harness window.rs::spawn_boot_then_window` |
| `WindowConfig` | — (new) | `harness window.rs` |

## Design-gate

- **DG-1 (reusability contract):** harness code names zero `ziee::`
  (machine-verified `grep -rn 'ziee::' sdk/desktop/harness/src | grep -v '//'`
  → empty). Deps = SDK crates + `tauri`/`tauri-plugin-decorum` only, never
  `ziee`.
- **DG-2 (equivalence):** window chrome (per-OS builders, effects, decorations,
  overlay, traffic-lights) moved verbatim; the boot skeleton preserves spawn +
  match + both-paths-create-window + the two log strings + the six post-boot
  steps' exact order and error strings. E8 golden byte-identical on both
  surfaces.
- **DG-3 (no shim owed):** the only moved symbol (`create_main_window`) was
  private with no external importer, so no old-path re-export is required.
