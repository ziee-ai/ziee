# DECISIONS — office_bridge desktop-only re-architecture

Every input resolved up front so implementation runs nonstop. All resolvable by codebase
convention + the user's stated directive (desktop-only); none require a fresh product call.

### DEC-1: How is the module registered from the desktop crate — `AppModule` via cross-crate `MODULE_ENTRIES`, or reshaped to a `DesktopModule`?
**Resolution:** Keep it an `AppModule` and register via `#[distributed_slice(ziee::module_api::types::MODULE_ENTRIES)]` defined in the desktop crate — preserving the tested `init(&ModuleContext)` lifecycle (upsert the `mcp_servers` row, spawn the bridge listener + watcher). Contingent on the ITEM-14 linkme cross-crate validation passing; if it fails, fall back to a `host_mount`-style `DesktopModule` + a manual MCP-registration seam.
**Basis:** codebase — `linkme` distributed slices are designed to aggregate entries across all crates in the final linked binary; this is the minimal-change path and keeps the already-audited init lifecycle intact. `host_mount` is the documented fallback pattern.

### DEC-2: How is the chat extension registered cross-crate?
**Resolution:** Via `#[distributed_slice(ziee::chat_extension::CHAT_EXTENSIONS)]` defined in the desktop crate (same mechanism as DEC-1), validated by the ITEM-14 smoke test. Fallback = a `pub fn ziee::register_chat_extension(entry)` runtime seam the desktop calls at boot.
**Basis:** codebase — identical linkme mechanism to MODULE_ENTRIES; one validation covers both.

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
