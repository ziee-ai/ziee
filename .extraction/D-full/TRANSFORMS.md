# Chunk D-full — TRANSFORMS

Every edit, why it is equivalence-preserving, and the decisions taken.

## T1 — `create_main_window` moved verbatim, literals → `WindowConfig`

**Was:** private `fn create_main_window(app_handle: &tauri::AppHandle)` in
ziee-desktop `modules/backend/mod.rs`, with five hardcoded literals: `.title("")`,
`.inner_size(1200.0, 800.0)`, `.min_inner_size(400.0, 600.0)`, effect
`radius: Some(8.0)`, `.traffic_light_position(LogicalPosition::new(20.0, 22.0))`.

**Now:** `pub fn create_main_window(app_handle: &tauri::AppHandle, config:
&WindowConfig)` in `sdk/desktop/harness/src/window.rs`. The five literals read
`config.{title,inner_size,min_inner_size,effect_radius,traffic_light_position}`.
**Every other token** — the macOS/Windows/Linux `#[cfg]` branches, the
Mica/Acrylic/Blur effect vecs, `decorations`, `TitleBarStyle::Overlay`, the
Windows decorum `create_overlay_titlebar` post-build block, both `tracing::info!`
lines — is byte-for-byte the original.

**Equivalence:** ziee constructs `WindowConfig { title: String::new(),
inner_size: (1200.0, 800.0), min_inner_size: (400.0, 600.0), effect_radius: 8.0,
traffic_light_position: (20.0, 22.0) }` — so `config.title.as_str() == ""` and
every field equals the former literal. Runtime output is identical. Window
construction is runtime-only (not exercised by the Bash-harness suite); this is
verified by construction + the unchanged callsite values, not a spawned window.

## T2 — boot→window skeleton generalized into `spawn_boot_then_window`

**Was:** inline in `start_backend_server`:
`tauri::async_runtime::spawn(async move { match ServerBoot::boot(&boot).await {
Ok(handle) => { info!("...on {}", handle.addr); <6 post-boot steps>;
state.set_ready(true); create_main_window(&app_handle); } Err(e) => {
error!("Failed to start backend server: {}", e); <comment>;
create_main_window(&app_handle); } } })`.

**Now:** the spawn + match + both-paths-`create_main_window` + the two log
strings live in `harness::window::spawn_boot_then_window(boot, app_handle,
window, on_ready)`. The app passes its six post-boot steps (incl.
`state.set_ready(true)`) as the `on_ready: FnOnce(BootHandle) -> impl Future`
closure, which the harness `.await`s between the success log and
`create_main_window`.

**Equivalence — ordering preserved exactly:**
`boot()` → `info!("Backend server started successfully on {}", handle.addr)` →
[on_ready: stash JWT_SERVICE, stash SERVER_POOL, `info!("JWT service stored…")`,
`run_desktop_migrations`, `host_mount::register_provider`, `ensure_desktop_admin`,
`backfill_system_mcp_assignments`, `enable_memory_admin_default`,
`state.set_ready(true)`] → `create_main_window`. Failure arm: `error!("Failed to
start backend server: {}", e)` (verbatim comment) → `create_main_window`. Same
sequence, same strings, same both-paths-window contract.

**Seam-type change:** `let boot = ZieeServerBoot::new(...)` (concrete, called via
`ServerBoot::boot(&boot)`) → `let boot: Arc<dyn ServerBoot> =
Arc::new(ZieeServerBoot::new(...))`. Behaviourally identical dynamic dispatch to
the same impl; the `Arc` is what lets the harness hold the seam for the run.

## T3 — harness `Cargo.toml`: add `tauri` + `tauri-plugin-decorum`

Version-matched to ziee-desktop (`tauri = "2.8" features ["tray-icon"]`,
`tauri-plugin-decorum = "1.1.1"`) so the single `src-app/Cargo.lock` unifies onto
one tauri build when ziee-desktop consumes the harness by path. `tauri` builds as
a plain library dependency (no `tauri-build`/`generate_context!` needed — those
stay in the `ziee-desktop` binary crate). Machine-verified: `cd sdk && cargo
check --workspace` compiles tauri + tauri-plugin-decorum + the harness, exit 0.

## T4 — harness `lib.rs` + ziee `mod.rs` imports/re-exports

- `lib.rs`: `pub mod window;` + `pub use window::{create_main_window,
  spawn_boot_then_window, WindowConfig};`; the module doc's BG-3-prerequisite
  deferral paragraph is replaced with the present-contents + kept-app-side
  rationale.
- `mod.rs`: `+ use ziee_desktop_harness::window::{spawn_boot_then_window,
  WindowConfig};`. The existing `use ...boot::ServerBoot;` is retained (needed for
  the `Arc<dyn ServerBoot>` annotation).

## Decision

**D-1 — Scope: which shell moves, given "equivalence-preserving MOVE, not
rewrite" + the STOP fail-safe?**
`create_main_window` is pure per-OS Tauri window construction with zero `ziee::`
references — it is the one cleanly-app-agnostic shell unit, so it moves verbatim.
The boot→window spawn *skeleton* is likewise generic and moves via an `on_ready`
closure seam. `run` / `run_headless` / `register_desktop_invoke_handler` / the two
IPC commands do NOT move: they wrap the app module system + `ziee::Config` +
`start_server_with_routes` ("the app's entire server assembly") and resolve
in-crate `#[tauri::command]` macros — moving them would force the harness to name
`ziee::`, violating the reusability contract. The task explicitly permits this
("keep tauri command registration app-side if domain-specific; move only
genuinely-generic glue").
**Resolution:** MOVE `create_main_window` + the boot→window skeleton
(`spawn_boot_then_window`) into the harness; KEEP `run`/`run_headless`/commands
app-side as thin consumers of the seam + the harness window shell. This is NOT a
STOP — the seam is threaded and works, the window/boot lifecycle now runs in the
harness, and the golden is byte-identical. The residual app-side `run` wrapper is
an inherent app/domain boundary (same rationale BG-3's `boot.rs` doc records),
not a broken tree.

**D-2 — Parameterize the window chrome or keep it fixed?**
Only the five values an app legitimately varies (title, sizes, radius, traffic-
light inset) are lifted into `WindowConfig`; the Mica/Acrylic/Blur effects,
decorations, overlay-titlebar and per-OS `#[cfg]` logic stay as harness code —
they are the shared desktop-shell *look* a second app (CytoAnalyst) should
inherit unchanged.
**Resolution:** minimal `WindowConfig` (5 fields); chrome logic fixed in the
harness. Equivalence holds because ziee passes exactly the former literals.

**D-3 — Re-export shim for `create_main_window`?**
It was a private `fn` (module-internal), called only from the two now-moved sites
in `start_backend_server`. No external path imports it.
**Resolution:** no shim needed (the shim rule applies only to symbols that must
stay importable from their old path).

_No `TBD` / `TODO` / `ASK` / `???` remain._
