# DECISIONS — office_bridge desktop-only re-architecture

Every input resolved up front so implementation runs nonstop. All resolvable by codebase
convention + the user's stated directive (desktop-only); none require a fresh product call.

### DEC-1: How is the module registered from the desktop crate — cross-crate `#[distributed_slice]`, or a runtime register seam?
**Resolution:** **RUNTIME SEAM** (revised after Phase-5 codebase evidence). Reshape office_bridge to a `host_mount`-style `DesktopModule` that, at boot, registers its REST/settings routes (`register_api_routes`), spawns the bridge listener + watcher and upserts the `mcp_servers` row using `ziee::Repos.pool()`, and registers its built-in MCP server + chat-extension + auto-attach entry via new `ziee::register_*` runtime functions. NO cross-crate `#[distributed_slice(ziee::…)]`.
**Basis:** codebase — the ONLY existing desktop→server downstream-registration in the repo is a runtime seam (`host_mount/mod.rs:82` → `ziee::code_sandbox::register_sandbox_mount_provider`, `mount_provider.rs:62`); there are ZERO `#[distributed_slice(ziee::…)]` registrations in the desktop crate. The "match existing patterns" project rule ([[feedback_match_existing_patterns]]) makes the runtime seam decisive — it is guaranteed to work and avoids linkme's cross-crate dead-code-linkage caveat, which is unproven here.

### DEC-2: How is the chat extension (and the auto-attach entry) registered from the desktop crate?
**Resolution:** Via runtime register functions in `ziee`, mirroring `register_sandbox_mount_provider`: `ziee::register_chat_extension(entry)` and `ziee::register_auto_attach_builtin(AutoAttachEntry)`, each appending to a `OnceLock<Mutex<Vec<…>>>` registry that the server consumes at boot (the chat-extension registry alongside `CHAT_EXTENSIONS`; `auto_attach_builtin_ids` alongside its slice/hardcoded arms). office_bridge's `DesktopModule::init` calls both. The `AUTO_ATTACH_BUILTINS` distributed slice added in Phase 5 (`e4776d6a`) is converted to / augmented by this runtime registry.
**Basis:** codebase — same `register_sandbox_mount_provider` precedent; guaranteed cross-crate delivery without linkme.

### DEC-3: Does the `AUTO_ATTACH_BUILTINS` inversion move ALL built-ins off the hardcoded list, or only office_bridge?
**Resolution:** Introduce the `AUTO_ATTACH_BUILTINS` distributed slice and iterate it IN ADDITION to the existing hardcoded arms in `auto_attach_builtin_ids`; move ONLY office_bridge's `{flag → server_id}` entry into it (registered from the desktop crate). Other built-ins keep their current handling.
**Basis:** convention — minimal blast radius ([[feedback_match_existing_patterns]]); only office_bridge's entry MUST leave `ziee` for the server to compile without the module.

### DEC-4: Does the `OfficeBridgeConfig` kill-switch move to the desktop, or stay in `ziee::Config`?
**Resolution:** Stays in `ziee::Config` as the existing `Option<OfficeBridgeConfig>` section (inert in server builds; read by the module via `ziee::Config`).
**Basis:** codebase — avoids churning the `Config` struct/schema; an unused optional section is harmless and the module already reads it through `ziee::Config`.

### DEC-5: Does `SyncEntity::OfficeDocument` move out of `ziee`?
**Resolution:** No — the variant stays in `ziee`'s `SyncEntity` enum; the desktop module references `ziee::SyncEntity::OfficeDocument`.
**Basis:** codebase — a downstream crate cannot add an enum variant; the enum drives the generated frontend `SyncEntity` TS union; the variant is inert (never emitted) in server builds.

### DEC-6: What migration numbers do office_bridge's migrations take in the desktop dir?
**Resolution:** `10000000000006_create_office_bridge.sql` + `10000000000007_grant_office_bridge_permissions_to_users.sql` (deleted from `server/migrations`). Fix the cosmetic "migration 133" mislabel in the grant's comment/warning.
**Basis:** codebase — next free in the desktop `1000…` space; disjoint from the server `0000…` space; grant runs after all server migrations so the Users group row exists.

### DEC-7: Where do the integration tests live and on which harness?
**Resolution:** `src-app/desktop/tauri/tests/office_bridge/`, on the desktop `TestServer` harness; if the desktop harness lacks `create_user_with_permissions`-style helpers the server harness has, add the minimal equivalent there.
**Basis:** codebase — `desktop/tauri/tests/` `host_mount_tests` is the precedent for a desktop-module integration suite.

### DEC-8: Does the WEB UI keep any office-bridge stub after the module moves to `desktop/ui`?
**Resolution:** No — the office-bridge UI module is fully removed from `src-app/ui` (dir moved to `desktop/ui`, dropped from the web UI module registry + e2e). The web app carries zero office-bridge.
**Basis:** user — the feature must be desktop-only.

### DEC-9: Does office_bridge stay behind per-call approval after the inversion?
**Resolution:** Yes — office_bridge stays OUT of `is_builtin_server_id`, and its `AUTO_ATTACH_BUILTINS` entry carries NO approval-bypass. Mutating office tools remain gated by approval.
**Basis:** codebase — preserves the original office_bridge feature's DEC-4 (mutating tool behind approval).

### DEC-10: This transforms the existing (already-committed) server-crate office_bridge — move or duplicate?
**Resolution:** A MOVE (git delete-from-server + add-to-desktop, rename-detected). The original `.lifecycle/office-bridge/` artifacts remain committed as branch history; this `office-bridge-desktop-only` feature dir tracks the re-architecture; both are stripped at merge to main per lifecycle hygiene.
**Basis:** convention — lifecycle artifacts are per-feature process records; product code is transformed, not duplicated.
