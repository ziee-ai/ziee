# PLAN — office_bridge desktop-only re-architecture

Move the entire `office_bridge` built-in MCP module OUT of the `ziee` server crate and INTO
the desktop crate, so it compiles/registers **only in the desktop binary** and is **entirely
absent from a plain `ziee` server build** (no COM, bridge listener, Office add-in assets,
migrations, permissions, routes, or MCP registration). Base = `office-bridge-desktop-only-base`
(the pre-refactor HEAD `c0ff6ac7`); this feature's diff is the re-architecture only.

Mechanism (proven by investigation): `ziee::MODULE_ENTRIES` and `ziee::…::CHAT_EXTENSIONS` are
`pub` `linkme` distributed slices — a module defined in the desktop crate registers into them and
is picked up by `ziee::create_modules()` in the desktop binary, while a standalone `ziee` server
binary (which never links the desktop crate) sees nothing. The desktop crate already owns a
`migrations/` dir (`1000…` space, applied by `run_desktop_migrations` + verified by the existing
build.rs loop), so office_bridge's schema rides that with no new migration infra.

## Items

- **ITEM-1**: Relocate the module tree `src-app/server/src/modules/office_bridge/` →
  `src-app/desktop/tauri/src/modules/office_bridge/` (mod, routes, handlers, models, tools,
  repository, permissions, watcher, bridge/*, platform/*, chat_extension/*) and declare it in
  `desktop/tauri/src/modules/mod.rs`. Registration mechanism (**DEC-1, revised to the runtime seam**):
  office_bridge becomes a `host_mount`-style `DesktopModule` — `register_api_routes` for its settings
  REST, and `init` spawns the bridge listener + watcher + upserts the `mcp_servers` row via
  `ziee::Repos.pool()`, and calls the new `ziee::register_*` runtime seams (ITEM-14). NO cross-crate
  `#[distributed_slice(ziee::…)]`. The COM/platform/bridge logic is unchanged.
- **ITEM-14**: Add the runtime registration seams in `ziee` (DEC-2), mirroring
  `code_sandbox::register_sandbox_mount_provider` (`OnceLock<Mutex<Vec<…>>>` fed by a `pub fn`):
  `ziee::register_chat_extension(entry)` (consumed at boot alongside `CHAT_EXTENSIONS`) and
  `ziee::register_auto_attach_builtin(AutoAttachEntry)` (consumed by `auto_attach_builtin_ids`).
  Convert/augment the `AUTO_ATTACH_BUILTINS` slice from Phase 5 into this runtime registry.
  office_bridge's `DesktopModule::init` calls both. The server binary, which never calls them, gets
  no office_bridge chat extension or auto-attach entry.
- **ITEM-2**: Rewrite the module's cross-crate references: `crate::…` server-framework paths →
  `ziee::…` (MODULE_ENTRIES, CHAT_EXTENSIONS, ModuleEntry/AppModule/ModuleContext, the MCP
  registration API, permission `PermissionCheck` trait, `common::secret`, repository factory,
  sync `publish`, error types, OpenAPI registration). Keep the two distributed-slice
  registrations (`#[distributed_slice(ziee::…MODULE_ENTRIES)]` order 97,
  `#[distributed_slice(ziee::…CHAT_EXTENSIONS)]` order 23).
- **ITEM-3**: Widen `ziee`'s crate-root public facade with the EXACT set the module needs (all
  already `pub` at their definition — only the `lib.rs` re-export is missing; ZERO private
  internals get exposed): `pub use modules::sync::{Audience, SyncAction, SyncEntity, publish}`;
  add `get_app_data_dir` to the existing app-state `pub use`; add the code_sandbox JSON-RPC types
  (`ConversationIdHeader, JsonRpcError{+from_app_error}, JsonRpcRequest, JsonRpcResponse`) to the
  `ziee::code_sandbox` facade; add a `ziee::chat_extension` facade
  (`ChatExtension, BeforeLlmAction, StreamContext, ExtensionEntry, ExtensionMetadata, CHAT_EXTENSIONS`
  + `request::SendMessageRequest`); add `model_supports_tools` to `ziee::file_available`.
- **ITEM-12**: Remove office_bridge from the `Repos` factory (`core/repository.rs:229`
  `office_bridge: OfficeBridgeRepository => …`) and rewrite the 3 call sites (`handlers.rs:336`,
  `handlers.rs:494`, `chat_extension/office_bridge.rs:76`) to construct
  `OfficeBridgeRepository::new(ziee::Repos.pool().clone())` — the exact `host_mount` precedent
  (`host_mount/mod.rs:81`, `host_mount/handlers.rs:22`).
- **ITEM-13**: `OfficeBridgeConfig` kill-switch — LEAVE the `Option<OfficeBridgeConfig>` section in
  `ziee`'s `Config` (an inert optional section in server builds; the downstream module reads it via
  `ziee::Config`). Fix the cosmetic "migration 133" mislabel in the grant migration's comment/warning.
- **ITEM-4**: Invert the hardcoded coupling in `ziee` `src/modules/mcp/chat_extension/mcp.rs`:
  add a `pub static AUTO_ATTACH_BUILTINS: [AutoAttachEntry]` distributed slice
  (`{ flag: &'static str, server_id: fn() -> Uuid }`); refactor `auto_attach_builtin_ids` to also
  iterate it; **remove** the direct `office_bridge::…`/`office_bridge_server_id()` reference from
  `ziee`. office_bridge registers its entry from the desktop crate. (Other built-ins keep their
  existing hardcoded handling — minimal blast radius; office_bridge's entry is the only one that
  MUST move so `ziee` compiles without it.)
- **ITEM-5**: Migrations — move `…133_create_office_bridge.sql` + `…134_grant_office_bridge…` from
  `server/migrations/` to `desktop/tauri/migrations/` renumbered into the `1000…` space
  (next free: `10000000000006_…`, `10000000000007_…`). Delete the server copies. Verify
  `run_desktop_migrations` (runtime) + the `server/build.rs` `../desktop/tauri/migrations` loop
  (compile-time sqlx) both pick them up.
- **ITEM-6**: `SyncEntity::OfficeDocument` — LEAVE the variant in `ziee`'s
  `src/modules/sync/event.rs` enum (an inert label in server builds; a downstream crate cannot add
  an enum variant). The desktop module keeps emitting it via `ziee`'s `sync::publish`.
- **ITEM-7**: Embedded add-in assets — move `src-app/server/resources/office-bridge/` into the
  desktop crate (`src-app/desktop/tauri/resources/office-bridge/`); update the `include_dir!` base
  path in the (relocated) `bridge/assets.rs`.
- **ITEM-8**: Frontend — move `src-app/ui/src/modules/office-bridge/` →
  `src-app/desktop/ui/src/modules/office-bridge/`. Module discovery is **glob-driven** (revised per
  DRIFT-1.2): `desktop-loader.ts` auto-picks the relocated dir and the web-ui glob drops it — NO
  registry edits (only the gallery coverage/state entries move web→desktop). Move the e2e spec too.
  The Open-Documents panel/card/store + e2e spec become desktop-UI only.
- **ITEM-9**: Tests — relocate the module's unit tests (move with the source) and the integration
  tests `server/tests/office_bridge/` (incl. TEST-9 live COM, TEST-7 bridge, settings/mcp) into the
  desktop crate's test tree (`desktop/tauri/tests/…`); ensure the desktop integration harness runs
  them and the desktop build DB has the office_bridge tables.
- **ITEM-10**: Regenerate BOTH OpenAPI specs + `types.ts`: the **web** spec
  (`ui/openapi/openapi.json` + `ui/src/api-client/types.ts`) loses all office_bridge routes/types;
  the **desktop** combined spec (`desktop/ui/…`) keeps them. Verify both `types_ts_parity` +
  `types_ts_parity_desktop` stay green.
- **ITEM-11**: Prove the negative: a plain `ziee` server build/spec contains **zero** office_bridge.
  The existing `app_builder.rs` module-count self-test naturally excludes it; add an assertion that
  the server (ui) OpenAPI has no `office` route and `MODULE_ENTRIES` (server-only link set) omits
  office_bridge.

## Files to touch

- **Delete from `ziee` server:** `src-app/server/src/modules/office_bridge/**` (whole tree),
  `src-app/server/migrations/00000000000133_create_office_bridge.sql`,
  `src-app/server/migrations/00000000000134_grant_office_bridge_permissions_to_users.sql`,
  `src-app/server/resources/office-bridge/**`, `src-app/server/tests/office_bridge/**`,
  and the office_bridge branch in `src-app/server/src/modules/mcp/chat_extension/mcp.rs`.
- **Edit in `ziee` server:** `src/modules/mcp/chat_extension/mcp.rs` (new AUTO_ATTACH_BUILTINS
  slice + iterate it), `src/lib.rs` / `src/module_api/**` (widen the public surface ITEM-3 needs),
  `src/modules/server/mod.rs` or wherever `create_modules` self-test counts modules.
- **Add to desktop crate:** `src-app/desktop/tauri/src/modules/office_bridge/**` (relocated tree),
  `src-app/desktop/tauri/migrations/10000000000006_create_office_bridge.sql` +
  `…0007_grant_office_bridge_permissions_to_users.sql`,
  `src-app/desktop/tauri/resources/office-bridge/**`, `src-app/desktop/tauri/tests/office_bridge/**`,
  register the module in `src-app/desktop/tauri/src/modules/mod.rs`.
- **Frontend:** move `src-app/ui/src/modules/office-bridge/**` →
  `src-app/desktop/ui/src/modules/office-bridge/**`; update both UIs' module registries;
  move the e2e spec `ui/tests/e2e/20-office-bridge/**` → `desktop/ui/tests/e2e/…`.
- **Regenerated (committed):** `src-app/ui/openapi/openapi.json`, `src-app/ui/src/api-client/types.ts`,
  `src-app/desktop/ui/openapi/openapi.json`, `src-app/desktop/ui/src/api-client/types.ts`, and the
  gallery/kit generated files in whichever workspace changed.

## Patterns to follow

- **Desktop server-side module registration:** there is no existing desktop module that registers
  a full `ziee` `AppModule` via `MODULE_ENTRIES` (existing desktop modules use the Tauri-side
  `DesktopModule` trait). Mirror the **server** built-in-MCP modules for the module SHAPE
  (`src-app/server/src/modules/web_search/` — the closest sibling: built-in MCP server + chat
  extension + settings + permissions), but place the files in the desktop crate and reference the
  framework via `ziee::` (the way `desktop/tauri/src/modules/remote_access/` already uses
  `sqlx::query_as!` against its own desktop-migration tables).
- **Desktop migrations:** mirror `src-app/desktop/tauri/migrations/10000000000003_create_remote_access_settings.sql`
  (the `1000…` numbering + `set_ignore_missing` coexistence) and its application via
  `run_desktop_migrations` (`desktop/tauri/src/modules/backend/mod.rs`).
- **AUTO_ATTACH_BUILTINS distributed slice:** mirror the existing `MODULE_ENTRIES` /
  `CHAT_EXTENSIONS` slice definitions (`module_api/types.rs`, `chat/core/extension/registry.rs`)
  for the `#[distributed_slice]` pattern.
- **Desktop OpenAPI merge:** already implemented in `src-app/desktop/tauri/src/openapi.rs`
  (`create_modules()` + `create_desktop_modules()` routes merged) — no change, just regenerate.
- **Frontend module in desktop UI:** mirror how `src-app/desktop/ui/` registers its modules
  (whatever the desktop-ui module registry is) and the existing shadcn office-bridge components
  (unchanged, just relocated).
