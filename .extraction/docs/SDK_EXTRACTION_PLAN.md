# ziee SDK — SDK-First Extraction Plan

**This is a PLAN document only.** No repo created, no code moved, no
`feature-lifecycle` started. Read it, approve/adjust, then we convert Phase-1 into
an executable plan.

**Reframe from the handoff doc (`ZIEE_PLATFORM_EXTRACTION_HANDOFF.md`).** SDK-FIRST.
Dropped: the `code_sandbox` "leaf pilot" / leaf-first ordering, and "always-green-**on-main**"
in-place refactoring. Replaced by: a standalone `ziee-ai/sdk` repo, submoduled into
`ziee-ai/ziee`, migrated on a **long-lived branch** (main untouched), MOVING code, green at
each chunk boundary.

**Scope — the SDK is META-FRAMEWORK + batteries-included IDENTITY + the LLM-CONTROL surface + UI
framework + desktop harness.** Surfaces: SERVER (`ziee-core` + `ziee-identity` + `ziee-framework` + `ziee-auth` + `ziee-control-mcp`),
SERVER-UI (`@ziee/framework` + `@ziee/kit`), DESKTOP (Tauri harness + dual-mode capability
manifest + owner-`*` permission model). **`control_mcp` is in v1** — a new app needs it to be
driven by the **ziee companion** (the app exposes `control_mcp`; ziee attaches as an external MCP
client and its LLM operates the app's own REST API). **v1 extracts the DB-free tool-dispatch core;
a new app's self-exposure needs the Tier-1 `mcp` registry — v1.5, decision N5.** **`code_sandbox` is out of SDK v1**
(extracted into the SDK in a later phase — decision #6). **Auth/user/group/permissions ARE now in scope** (this revision) — see §1.3.

**Provenance.** File:line refs come from the read-only traces in the handoff doc against `ziee`
@ `main` `7575548a4`. DESKTOP file:line detail (§1.3, Chunk D) awaited a dedicated trace at
write time (flagged where thin).

---

## Contents
1. Repo structure — the `sdk` repo layout + exactly how ziee consumes it
2. Migration model — long-lived branch, moved-not-copied, green-at-boundary
3. Chunk sequence — what moves, in order, with the design-gate each resolves
4. Early skeleton second-consumer milestone
5. Submodule-pointer workflow (`just sync-sdk`)
6. Final phase — the all-at-once app consolidation (ziee thin; CytoAnalyst new)
7. Build-time SQLx, Docker, the build DB & **migration composition**
8. Open questions needing human decision
9. Testing & regression strategy — proving ziee (incl. chat) is unchanged
10. Extraction gates & hardening — the feature-lifecycle analog
11. SDK capability catalog & roadmap

---

## 1. Repo structure

### 1.1 Two repos, one submodule
- **`ziee-ai/sdk`** — standalone, independently build/test-able; single source of truth for the
  platform + identity layer.
- **`ziee-ai/ziee`** — consumes the SDK as a **git submodule at `sdk/`**, pinned to an exact SDK
  commit, referencing SDK crates/packages by **path** into the submodule tree.

### 1.2 SDK repo internal layout (5 crates — auth layering + LLM-control)

```
sdk/
├─ Cargo.toml                 # SDK's OWN workspace: members = ["crates/*","desktop/*","examples/*"]
├─ package.json               # SDK's OWN npm workspace root: workspaces = ["packages/*"]
├─ justfile
├─ crates/
│  ├─ ziee-core               # AppError/ApiResult, macros, base app-state globals
│  │                          #   (configurable app-name), ServerConfig.   [build-DB-free]
│  ├─ ziee-identity           # IDENTITY ABSTRACTIONS: Principal trait, JWT-verify interface,
│  │                          #   PermissionCheck/List + wildcard/is_admin eval, refresh-token
│  │                          #   verify. Depends on ziee-core.            [build-DB-free*]
│  ├─ ziee-framework          # module_api (AppModule/ModuleContext/MODULE_ENTRIES), app_builder,
│  │                          #   aide with_permission, emit_ts generator + openapi driver, sync
│  │                          #   core (+ SyncEntityKind), declare_repositories!, EventBus, and
│  │                          #   RequirePermissions built on ziee-identity. [build-DB-free]
│  ├─ ziee-auth               # DEFAULT (optional) AUTH MODULE, schema-bound: users/groups/permissions/
│                             #   refresh_tokens/sessions/session_settings TABLES + queries +
│                             #   login/register/LDAP/OAuth2 + admin CRUD + Session&Token-Refresh
│                             #   + dual-mode auth strategies (multi-user + single-user
│                             #   auto-login/owner-*). Owns its migrations.   [SCHEMA-BOUND → DB]
│  └─ ziee-control-mcp        # LLM-CONTROL SURFACE (v1): app's OpenAPI → 3 MCP tools (list/
│                             #   describe/invoke_capability); forwarded-JWT loopback re-auth +
│                             #   secret denylist + mutation-approval + permission filter. How the
│                             #   ziee companion drives a new app. Depends on framework + identity;
│                             #   JSON-RPC/loopback scaffolding lives in framework. [build-DB-free]
├─ packages/
│  ├─ framework               # @ziee/framework — core/* runtime + ApiClient runtime + sync client
│  └─ kit                     # @ziee/kit — shadcn kit + tokens
├─ desktop/
│  └─ harness                 # ziee-desktop-harness (Rust): Tauri shell embedding a framework
│                             #   server + embedded Postgres + auto-login single-user strategy
└─ examples/
   └─ skeleton-server         # the permanent second-consumer guard (§4)
```

Dependency direction (bottom→top): `ziee-core` → `ziee-identity` → `ziee-framework` →
`ziee-auth` (a module). This **breaks the circular dependency**: enforcement in the framework is
generic over the identity *abstractions* (`ziee-identity`, below); the concrete auth *module*
(`ziee-auth` — the **default, replaceable** implementation, routes + tables) registers via the
module system, so it sits **above** it. **Identity is PLUGGABLE (decision #1):** an app may use
`ziee-auth` or supply its own impl of `ziee-identity`'s traits.

### 1.3 The surfaces in detail

- **SERVER (Rust, 4 crates).**
  - `ziee-core` — foundation; `AppError`/`ApiResult` (`common/type.rs:28`, **323 files**).
  - `ziee-identity` — identity **abstractions**: a `Principal`/identity trait, the
    `PermissionCheck`/`PermissionList` traits + RBAC wildcard/`is_admin` eval, and a JWT-verify
    interface — generic, **no concrete tables**. `ziee-framework`'s `RequirePermissions`
    (`permissions/extractors.rs:105+`) is generic over an **injected identity resolver** that
    implements these traits (**pluggable identity**, decision #1).
  - `ziee-framework` — module machinery (`module_api/backend_module.rs:10-37`, `types.rs:20-32`,
    `core/app_builder.rs:17-124`), codegen generator (`openapi/emit_ts.rs`, 1323 lines), sync
    core (`sync/registry.rs`, `event.rs`; `ClientConn` `registry.rs:42-48` now consumes
    `ziee-identity`'s `User`/`Group`), the `declare_repositories!` macro (`repository.rs:26-190`),
    and the EventBus.
  - `ziee-auth` — the **default (optional, replaceable)** schema-bound MODULE that implements
    `ziee-identity`'s traits: users/groups/permissions/`refresh_tokens`/sessions/`session_settings`
    tables + `query!` macros + login/register/LDAP/OAuth2 + admin CRUD + the full Session & Token
    Refresh subsystem + the dual-mode auth strategies. Registered like any module; **owns its
    migrations** → the SDK's one build-DB-requiring crate (§7). Permission **STRINGS**
    (`knowledge_base::use`, etc.) are **app-registered** into a registry replacing
    `all_permissions()` (`user/permissions.rs:143`); the RBAC *engine* is in `ziee-identity`, the
    tables in `ziee-auth`. An app may swap `ziee-auth` for its own identity impl.
- **SERVER-UI (TS).** `@ziee/framework` = domain-agnostic runtime — `core/store-kit.ts`,
  `core/stores.ts` (`createStoreProxy`), `core/module-system/store.ts`, events, overrides, router
  module, sync client (`core/sync/SyncClient.ts`), + ApiClient **runtime** (`api-client/core.ts`).
  `@ziee/kit` = `components/ui/*`. Generated `api-client/types.ts` (`types.ts:5760` `SyncEntity`)
  is **per-app OUTPUT**.
- **DESKTOP (Rust harness + framework config).** `sdk/desktop/harness` factors the "Tauri-embeds-
  server + embedded-Postgres + auto-login permanent session" shell out of `src-app/desktop/tauri`.
  It **selects the single-user auth strategy from `ziee-auth`** (auto-login/owner-`*`). Two
  dual-mode abstractions live in `ziee-framework`/`ziee-auth` and are selected by the harness:
  (1) a **per-app capability manifest keyed by mode** replacing the **four** mode-gating mechanisms
  (Chunk D); (2) the **owner-holds-`*`-wildcard** single-user permission strategy
  (via the existing `is_admin` short-circuit), so permission-gated code is written once and runs
  in both modes. On the UI side, desktop swaps components via the **overrides seam**
  (`core/overrides/registry.ts:24`, augmentable `UIOverrides`) — framework machinery in
  `@ziee/framework` (Chunk F2); the `desktop/ui/` bundle registers desktop-specific components at
  boot. Override selection keying is open (§8 #12). Evidence dual-mode works cross-app:
  `desktop/ui/vite.config.ts:24` aliases `@/`→`ui/src/`; `desktop/ui/src/modules/window/store.ts:5`
  imports `defineStore` from `@/core/store-kit`. Full desktop file:line map + harness partition in **Chunk D**.

### 1.4 Exactly how ziee consumes the SDK

**Submodule.** `.gitmodules` registers `sdk/` → `git@github.com:ziee-ai/sdk`, pinned to a commit;
CI runs `git submodule update --init --recursive`.

**Cargo — PATH DEPS into `sdk/crates/*`** (path deps, not workspace members — §8 #1):
```toml
ziee-core            = { path = "../../sdk/crates/ziee-core" }
ziee-identity        = { path = "../../sdk/crates/ziee-identity" }
ziee-framework       = { path = "../../sdk/crates/ziee-framework" }
ziee-auth            = { path = "../../sdk/crates/ziee-auth" }
ziee-desktop-harness = { path = "../../sdk/desktop/harness" }   # desktop crate only
```
Config resolution (`.cargo/config.toml`, `POSTGRESQL_VERSION`, static-libseccomp) is taken from
the invocation dir (ziee root), so it applies to the whole graph. Shared-dep version alignment
between the two workspaces is a discipline (§8 #1).

**npm — workspaces include `sdk/packages/*`:**
`"workspaces": ["src-app/ui","src-app/desktop/ui","sdk/packages/*"]`. The SDK's own
`sdk/package.json` workspace root is used only for standalone SDK dev.

---

## 2. Migration model

### 2.1 Invariants
1. **`ziee-ai/ziee` `main` stays green/untouched** until the end. Work lives on a **long-lived
   branch** (`feat/sdk-extraction`); the SDK develops in tandem; the ziee branch pins SDK commits.
2. **MOVED, not parallel-copied.** Each chunk is cut out of ziee INTO the SDK (history preserved),
   its tests ported into the SDK, ziee wired to the path dep. One source of truth at every boundary.
3. **Green at each chunk boundary** (ziee compiles + tests; SDK compiles + tests). Red mid-chunk is fine.
4. **Git code-history preserved** (one up-front `filter-repo`). **Migration-chain history is NOT** —
   N3.1 deliberately squashes the 137+10 numeric chain into clean per-module baselines (no deployed
   DBs to protect, N8). The two are distinct: source-file lineage survives; the migration sequence
   is reconstructed. Equivalence is proven at the *result* (schema + seed), not the chain (§10 EA).

### 2.2 The per-chunk "cut"
1. Files are brought into the SDK by **one up-front `git filter-repo`** of the whole platform slice
   (history preserved once, decision #3); each chunk then **refactors in-place in the SDK** rather
   than doing 12× incremental history-moves.
2. Resolve the chunk's design-gate in the SDK (genericization/split).
3. Port its tests into the SDK.
4. Delete from ziee + wire the path dep + bump the submodule pointer.
5. Green the boundary (`just sync-sdk`, §5).

> Don't start the next chunk's SDK-side work until the current chunk's step 4 (delete-from-ziee)
> is done, or a refactored SDK copy and an un-deleted ziee copy would **diverge**.

---

## 3. Chunk sequence

Backend and frontend are separate build graphs (parallelizable after Chunk 0); desktop depends on
the backend framework + auth. Each chunk: **what moves** (file:line), the **design-gate**, the
**green criterion**.

### Chunk 0 — SDK skeleton + wiring bootstrap
Create the `sdk` repo with **empty** crates/packages; add the submodule; wire Cargo path deps +
npm workspaces; add `just sync-sdk`. **Gate:** none — proves the nested Cargo/npm workspace
plumbing (§8 #1). **Green:** ziee compiles/tests unchanged with the empty SDK linked.

### BACKEND TRACK

**Chunk B1 — `ziee-core` foundation.** *Mechanical.*
- **Moves:** `AppError`/`ApiResult` (`common/type.rs:28`, 323-file import rewrite), core macros,
  base app-state globals (configurable app-name). Define **`ServerConfig`** here.
- **Green:** ziee compiles against `ziee_core::AppError`; suite passes.

**Chunk B1b — `ziee-identity` abstractions.** *Design-gate: pluggable identity.*
- **Moves:** the identity **traits** — a `Principal`/identity trait, `PermissionCheck`/
  `PermissionList` + RBAC wildcard/`is_admin` eval, and a JWT-verify interface — into
  `ziee-identity` (depends on `ziee-core`). **No concrete tables/types** (those live in the
  default impl, `ziee-auth`).
- **Gate:** the framework enforces against these traits; the concrete identity is **injected**
  (pluggable, decision #1), so an app can provide its own. build-DB-free.
- **Green:** ziee references `ziee_identity` traits; compiles; suite passes.

**Chunk B2 — module system + Config split.** *Design-gate: Config.*
- **Moves:** `module_api/*` + `core/app_builder.rs:17-124` into `ziee-framework`.
- **Gate — Config split:** `ModuleContext` carries only `ServerConfig`; ziee's monolithic `Config`
  (`config.rs:5-45`) composes `ServerConfig` + domain sub-configs.
- **Green:** ziee registers modules against the moved `MODULE_ENTRIES`; boots; suite passes.

*(→ EARLY SKELETON MILESTONE runs here — §4 — before any further chunks.)*

**Chunk BG — de-globalize `Repos` / JWT / config.** *Design-gate: remove global singletons (blocks B3+).*
- **Refactors (in-ziee, no crate moves yet):** the global `ziee::Repos` static, the JWT `OnceLock`,
  and the config statics are why the framework can't be generic. Thread them (or place behind
  context/harness traits) so `RequirePermissions`, `ModuleContext`, and the desktop harness take
  them as parameters, not globals.
- **Gate:** ziee still boots; every global resolved to threaded/trait form; **E8 golden identical**
  (pure refactor, no wire change).
- **Green:** full suite green; framework-bound code no longer references `crate::` globals. Unblocks
  B3/B4/BA/D. *(Decision N6 — runs before B3.)*

**Chunk B3 — permission enforcement (pluggable).** *Design-gate: injected identity resolver.*
- **Moves:** `with_permission` (`permissions/openapi.rs`) + `RequirePermissions`
  (`permissions/extractors.rs:105+`) into `ziee-framework`, **generic over an injected identity
  resolver** implementing `ziee-identity`'s traits (pluggable, decision #1) — the SDK never
  hardcodes a concrete `User`/`Group`.
- **Gate:** permission **STRINGS** stay **app-registered** via a registry replacing
  `all_permissions()` (`user/permissions.rs:143`); ziee wires its `ziee-auth`-backed resolver.
- **Green:** ziee registers its resolver + permission vocabulary; auth/permission tests pass.

**Chunk B4 — `declare_repositories!` macro.** *Mechanical.*
- **Moves:** the macro (`repository.rs:26-190`) into `ziee-framework`; the repo **list**
  (`:198-233`) + 171 `Repos.xxx` sites **stay in ziee**.
- **Green:** `Repos` builds from the SDK macro; suite passes.

**Chunk B5 — sync core + SyncEntity extensibility.** *Design-gate: SyncEntity.*
- **Moves:** `sync/registry.rs`, `sync/event.rs` machinery + EventBus into `ziee-framework`.
- **Gate:** the closed 55-variant `SyncEntity` (`event.rs:28-217`) becomes an app-extensible
  `SyncEntityKind` (still derives `JsonSchema` for the codegen contract); ziee keeps its enum.
- **Green:** ziee entities flow through; sync tests + cross-device behavior pass.

**Chunk B6 — emit_ts generator + codegen contract.** *Design-gate: codegen.*
- **Moves:** `openapi/emit_ts.rs` (1323 lines) + the openapi driver (`openapi/mod.rs:12-115`) into
  `ziee-framework`; parameterize the output path (`openapi/mod.rs:100`).
- **Gate:** generator in SDK; generated `types.ts`/ApiClient stay **per-app OUTPUT**; the
  generator golden test moves to the SDK (fixture-based, `emit_ts.rs:1264-1323`); each app keeps a
  regen-drift guard.
- **Green:** `just openapi-regen` in ziee yields byte-identical `types.ts`; SDK + ziee guards pass.

**Chunk BA — auth module → `ziee-auth` (schema-bound).** *Design-gates: schema extraction +
migration composition.*
- **Moves:** the users/groups/permissions/`refresh_tokens`/sessions/`session_settings` TABLES +
  `query!` macros + login/register/LDAP/OAuth2 + admin CRUD + the full Session & Token Refresh
  subsystem + the **dual-mode auth strategies** (multi-user + single-user auto-login/owner-`*`)
  into the `ziee-auth` MODULE crate (depends on `ziee-framework` + `ziee-identity`).
- **Gate — migration composition:** `ziee-auth` owns its migrations; the app runs a **merged
  Migrator** (SDK-auth ∪ app, sorted by version). *(BA's initial carve-out kept the original numeric
  files; the later **MIGRATE-squash** chunk (N3.1) reconstructs ALL migrations to squashed,
  module-owned, `<YYYYMMDDNNNN>`-versioned baselines — no version/checksum preservation, since N8
  confirms no deployed DBs. Equivalence is proven by EA's logical fingerprint + whole-DB seed, §10,
  NOT by chain preservation.)* SDK CI gets an **auth-only build DB** to verify `ziee-auth`'s `query!`.
- **Green:** ziee boots with auth served from `ziee-auth`; auth/session/permission integration +
  E2E pass; the merged migrator applies cleanly on a fresh DB (EA fingerprint + seed equivalent).

**Chunk C1 — `ziee-control-mcp` capability (v1).** *Design-gate: the LLM-control surface.*
- **Moves:** `control_mcp` (`modules/control_mcp/{catalog,policy,tools}.rs` — tool-dispatch only;
  `handlers`/`routes`/`repository`/`chat_extension` + the `mcp_servers` row stay app-side) into a
  `ziee-control-mcp` crate — the OpenAPI→catalog ingest (`catalog.rs:91 init_from_openapi`), the 3
  tools (`list_capabilities`/`describe_capability`/`invoke_capability`), forwarded-JWT loopback
  invoke, secret-body denylist, path-param hardening, and the permission filter. The shared
  MCP-server scaffolding (JSON-RPC types + `loopback_host`, dependency-free) moves into
  `ziee-framework`.
- **Gate:** the catalog builds from the app's SDK-generated OpenAPI (needs Chunk B6); the permission
  filter uses the injected identity resolver (Chunk B3). The **tool-dispatch core is build-DB-free**;
  the module's `mcp_servers`-row registration (`repository.rs:29`) + `routes`/`chat_extension` stay
  **app-side** in v1.
- **Green:** ziee's `control::` integration/permission tests pass. **Fresh-app self-exposure is
  descoped to v1.5** (needs the Tier-1 `mcp` registry; v1 extracts only the tool-dispatch core — N5).

### FRONTEND TRACK (parallelizable after Chunk 0)

**Chunk F1 — `@ziee/kit`.** Move `components/ui/*` (+ `testIds.generated.ts`) into
`sdk/packages/kit`. Also proves npm-submodule wiring. **Green:** ziee UIs import `@ziee/kit`; gate:ui.

**Chunk F2 — `@ziee/framework` runtime.** Move `core/module-system/*`, `core/module.ts`,
`core/events/*`, `core/stores.ts`, `core/store-kit.ts`, `core/overrides/*`, the router module,
`core/sync/*` runtime, and the ApiClient runtime (`api-client/core.ts` + `index.ts` + `getBaseURL*`
+ `sse-types`) into `sdk/packages/framework`. **Gate:** define framework-side interfaces the per-app
generated `types.ts` satisfies; harden declaration-merge across the package boundary. **Green:**
ziee `ui/` + `desktop/ui/` author against `@ziee/framework`; tsc + gate:ui pass in both.

### DESKTOP TRACK (after backend framework + auth)

**Chunk D — desktop harness + dual-mode.** *Design-gates: capability manifest + single-user strategy.*
The desktop app is one crate `ziee-desktop` (`src-app/desktop/tauri`) depending on the `ziee` server
crate as a library; `lib.rs::run` starts a Tauri shell and in `.setup` **spawns the Axum server
in-process** (`backend/mod.rs:228 start_backend_server` → `ziee::start_server_with_routes`,
`server/lib.rs:603`, detached `axum::serve`). **IPC is exactly 2 Tauri commands**
(`get_server_port`, `auto_login`) — everything else is HTTP (confirms Axum-over-invoke).
- **Moves → `sdk/desktop/harness`:** the Tauri-shell boot skeleton (`lib.rs::run`/`run_headless` +
  `register_desktop_invoke_handler` + per-OS `create_main_window`/titlebar, `backend/mod.rs:803-892`),
  the **embed-server glue** (`start_backend_server` spawn + the `start_server_with_routes` closure
  that re-layers CORS/JWT + stashes the JWT `OnceLock`), and the **single-user auto-login strategy**
  (`mint_admin_login`/`auto_login` `auth/commands.rs:61` → same `mint_session_tokens` path;
  `ensure_desktop_admin` `auth/bootstrap.rs:8`; per-boot JWT-secret policy `backend/mod.rs:526`) —
  **parameterized over the `ziee-identity` resolver** (pluggable), `ziee-auth` supplying the concrete
  `"admin"` impl. The 2 Tauri commands are harness-provided. The **embedded-Postgres lifecycle**
  (`server/database/mod.rs:128-429`, selected by `postgresql.use_embedded`) relocates into
  **`ziee-framework`'s DB bootstrap** (generic — zero-config server uses it too), NOT the harness.
- **Stays app-side:** `create_desktop_modules` module vec (`core/module_builder.rs:22-42`) + the
  desktop-only modules (remote_access, magic_link, tunnel_auth, host_mount, tray, updater), the
  feature-flag overrides (`backend/mod.rs:147-169`), the `CORE_MODULE_BLOCKLIST` **contents**, the
  config YAML template, CORS allowlist + branding, the frontend override *impls*.
- **Gate — the FOUR-part capability manifest.** Mode-gating today is four separate things a single
  per-app manifest replaces: (1) the backend hard-coded module vec `create_desktop_modules`; (2) the
  **frontend** `CORE_MODULE_BLOCKLIST` Set (`ui/src/modules/loader.desktop.ts:33` — a *forked loader*
  via Vite alias, not a cargo feature); (3) the scattered `config.<feature>.enabled=true` overrides
  (`backend/mod.rs:147-169`); (4) `setMultiUserMode(false)` (`desktop/ui/main.tsx:52`). Plus formalize
  the single-user strategy + owner-`*` (`is_admin` short-circuit `permissions/extractors.rs:121-128`,
  `"*"` wildcard `permissions/checker.rs:38-52`) as `ziee-auth`/`ziee-identity` strategies.
- **Worst 3 couplings (medium effort):** (1) the global **`ziee::Repos` singleton** (desktop reaches
  `Repos.user`/`.app`/`.pool()` directly) → behind a harness trait; (2) the JWT `OnceLock` + config
  statics; (3) the **`"admin"`/`is_admin` schema assumption** baked into the single-user identity →
  route through the pluggable identity abstraction. Behaviorally well-isolated (headless-parity
  already forces a stable boundary). Embedded-PG relocation out of the server crate is the other task.
- **Green:** ziee-desktop boots on the harness; the 2 IPC commands + auto-login + permanent-session
  desktop E2E pass; the server build is unaffected.

**Chunk MIGRATE-squash — migration reconstruction (N3.1 / N7 / N8 / N9).** *Runs AFTER D-full.*
- **CUT/reshape:** squash the composed 147-migration history into clean per-module baselines. Each
  baseline lands in its **owning module's** `migrations/` dir: `sdk/crates/ziee-auth/migrations/`
  (auth tables + CLEAN perms: `Admin=['*']`, `Users=['profile::*']`); each ziee module gains
  `src-app/server/src/modules/<mod>/migrations/` (chat, llm, mcp, memory, files, file_rag, workflow,
  code_sandbox, project, citations, knowledge_base, web_search, lit_search, sync, …). The
  `chat::/branches::/assistants::/mcp_servers::/hub::/files::/conversations::` seed UPDATEs move to
  the owning module's data migration. Naming: `<timestamp>_<module>_<desc>.sql`, dependency-ordered.
- **build.rs:** widen `compose_merged_migrations()` source globs from the single `migrations/` dir to
  `modules/*/migrations/ ∪ sdk/crates/*/migrations/`, timestamp-sort, compose `migrations-merged/`.
  Add a framework migration-authoring convention doc.
- **Gate (EA-revised) — TWO LOGICAL equivalence anchors, schema AND data (NOT byte-identical, B1):**
  1. **Schema fingerprint (catalog-derived, name/order-invariant).** A squash reorders `CREATE TABLE`
     vs later `ALTER ADD COLUMN`, so `pg_dump --schema-only` byte-differs on a *logically identical*
     schema (attnum order, auto constraint-name suffixes, emission order). So the gate builds a
     **structural fingerprint** from `information_schema`/`pg_catalog` on BOTH the numeric-baseline DB
     and the squashed DB: each table as a SET of `{column → (data_type, is_nullable,
     normalized-default, **is_generated + generation_expression**)}` (order-independent — the
     generated expr is REQUIRED; ziee has 3 `GENERATED` cols); indexes via name-normalized
     `pg_get_indexdef` (opclass e.g. `halfvec_cosine_ops` + predicate + method); constraints via
     `pg_get_constraintdef` (FK target+cols+`ON UPDATE/DELETE`+`DEFERRABLE`, CHECK, PK/UNIQUE);
     enums/sequences/functions/triggers/extensions by **definition, not auto-name**. Diff must be
     EMPTY. Validator RE-RUNS the fingerprint script on both DBs (unfakeable). **Exact catalog
     columns: SPEC §3 EA-schema.**
  2. **Whole-DB seed data.** A fresh-migrated DB holds ONLY seed rows (no user data), so compare the
     ENTIRE data image, not a curated table list (H3). FIRST capture `.extraction/baseline/seed.sql`
     = all rows of all tables from a fresh **numeric**-migrated DB. After the squash, compare
     **per-table by business key** (NOT literal uuid — the baseline seeds some FK'd rows with random
     `gen_random_uuid()`): drop volatile cols (`created_at`/`updated_at`, surrogate uuid PKs), key
     rows by natural cols (`groups.name`, `auth_providers.name`, settings discriminators), resolve
     FKs **through the referenced business key** (join, don't compare raw uuids), and **element-sort
     set-valued arrays** (`groups.permissions TEXT[]` — order not significant). Missing/extra/
     mismatched row fails. **Exact algorithm: SPEC §3 EA-seed.**
  - Golden types/openapi trivially unchanged (migrations aren't in the API surface). **N9
    seed-assertion:** `grep` the auth migrations → zero non-`profile::`/non-`*` perm strings. Build
    DB provisions clean; `cargo check -p ziee` green (all `query!` still verify against the merged
    schema). Immutability re-established from the new baseline forward.
- **Green:** ziee boots on the squashed set; schema-identical; auth crate domain-clean; each module
  owns its schema.

---

## 4. Early skeleton second-consumer milestone

**Placement:** immediately after **Chunk B2** — before B3–B6/BA, the frontend beyond F1, or
desktop. `sdk/examples/skeleton-server` depends on **ONLY** `ziee-core` + `ziee-framework` (NOT
`ziee-auth`), registers **one module + one route**, builds its own `types.ts` via the SDK
generator, and boots — linking **zero** of ziee's domain crates. **Gate:** if it can't build/boot
without ziee's domain (or without `ziee-auth`), the boundary leaked → fix before proceeding.
**Keep it in SDK CI forever** as the executable definition of "app-agnostic." (It also proves the
framework is usable **without** the auth module — auth is opt-in.)

---

## 5. Submodule-pointer workflow (`just sync-sdk`)

```
just sync-sdk:
  1. cd sdk && git add -A && (commit if dirty) && git push
  2. cd sdk && cargo test --workspace && npm test   # SDK green standalone
  3. cd .. && git add sdk                            # stage the new pointer
  4. print old_pointer → new_pointer + ziee git status
```
Companions: `just sdk-status` (pinned sha vs SDK tip), `just sdk-checkout <sha|branch>`; both CIs
run `git submodule update --init --recursive` and fail if the pinned SDK commit doesn't build.
When complete + green: SDK branch → SDK `main`; ziee `feat/sdk-extraction` (pinning SDK `main`) → ziee `main`.

---

## 6. Final phase — the all-at-once app consolidation

Trigger: **SDK feature-complete + fully tested** (all chunks on SDK `main`; `skeleton-server`
green; generator + drift guards green; auth merged-migrator green on fresh+existing DBs; desktop
harness green). By construction ziee's branch already consumes the SDK for the whole platform +
identity layer, so:
1. Merge the SDK branch to `ziee-ai/sdk` main; tag a baseline.
2. Merge `feat/sdk-extraction` to `ziee-ai/ziee` main in one shot — ziee is now a **thin domain
   layer**: `src-app/server/src/modules/*` (chat, LLM, memory, MCP, scientific tools) + its
   `Config`, `SyncEntity`, `Repos` list, permission vocabulary, and generated `types.ts` on the SDK
   (auth/users/permissions now served by `ziee-auth`).
3. **Bootstrap CytoAnalyst as a NEW app** from the same template: submodules the SDK (or consumes
   published artifacts — §8 #4), provides its own `modules/*` (studies/embedding/clustering/DE/…),
   `Config`, `SyncEntity`, `Repos`, generated `types.ts`, ECharts-GL UI, its own MCP server, the
   `QueueUtils`→sandbox shim — and gets **users/auth/RBAC + dual-mode desktop for free** from the SDK.

---

## 7. Build-time SQLx, Docker, the build DB & migration composition

**Design principle: the FRAMEWORK crates are build-DB-free; `ziee-auth` is the one exception.**
An app-agnostic framework can't hold compile-time `sqlx::query!` macros (they verify against a
specific schema). So `ziee-core` + `ziee-identity` + `ziee-framework` compile/test with **no build
DB, no docker, no migrations**. **`ziee-auth` is schema-bound** — it owns tables + `query!` macros
+ migrations, so it needs a build DB, isolated to its own SDK-CI lane. `ziee-control-mcp`'s **tool-dispatch core**
(`catalog/policy/tools`) is **build-DB-free** — it reads the OpenAPI catalog in memory and invokes
routes over the loopback. But the full `control_mcp` module writes an `mcp_servers` row at init
(`repository.rs:29`), so that registration + `routes`/`chat_extension` **stay app-side in v1** (a
new app self-exposes control only with the Tier-1 `mcp` registry — v1.5, decisions N1/N5).

Concretely: `ziee-framework` uses sqlx *types* (`PgPool`, `FromRow`) and ships the
`declare_repositories!` macro, which **expands inside the app crate** (its `query!` verify against
the **app's** build DB). `ziee-identity` stays build-DB-free by taking DB loading as a trait/callback.

**What stays in ziee (each app owns), unchanged:** `server/build.rs` +
`server/build_helper/worktree_db.rs` (per-worktree `ziee_build_<key>`), the app's `migrations/`,
`docker-compose.yaml` (`pgvector/pgvector:pg17`, `:54321`/`:54322`).

### 7.1 Migration composition — the merged Migrator (RATIFIED)

**Mechanism.** `ziee-auth` embeds its migrations and exports a `Migrator`:
```rust
pub static AUTH_MIGRATOR: Migrator = sqlx::migrate!("migrations");   // in ziee-auth
```
Each app builds **one merged Migrator** = SDK-auth migrations ∪ app migrations, **sorted by
version**, used in BOTH places that touch schema: app-startup `migrate run` AND `build.rs` when it
provisions `ziee_build_<key>`. **Mechanism (decision N3):** runtime Migrator concatenation is **not a supported sqlx API**, so the
app **composes one migration directory at build time** (SDK-auth ∪ app, the SDK migrations embedded
from `ziee-auth`) and runs `sqlx::migrate!` over it — which also updates `build.rs` + the `migrate!`
call site.

**Reconstruction (N3.1 — supersedes the numeric-preserve model below the line).** Because there are
NO deployed DBs to protect (N8), the 137+10 numeric history is **squashed** into clean per-module
baselines. **ALL** migrations become `<YYYYMMDDNNNN>_<module>_<desc>.sql` — a date prefix + a
**monotonic counter** `NNNN` (NOT wall-clock seconds, which collide when 147 files are authored in
one session). The counter is assigned to preserve **dependency order** (a valid topological order:
every FK-target table's migration sorts before its referrer). Migrations are **module-owned** (N7):
build.rs globs `modules/*/migrations/ ∪ sdk/crates/*/migrations/`, version-sorts, composes
`migrations-merged/`, and `sqlx::migrate!`s over it.

**Shared-table ownership map (H4 — one owner per table, others FK in):**
`users`/`groups`/`sessions`/`auth_providers`/`refresh_tokens`/`user_auth_links` → `ziee-auth`;
`files` → files module; `file_chunks`/`file_index_state` → file_rag; join tables (`project_files`,
`*_knowledge_bases`, `project_bibliography`, …) → their parent module; `CREATE EXTENSION vector` →
a framework/core **bootstrap** migration that sorts FIRST; a domain permission-grant seed → the
feature-owning module (this is where the old `27_fix_default_user_permissions` `chat::`/`branches::`
UPDATE lands — NOT in `ziee-auth`, per N9).

**Equivalence gate is LOGICAL, not byte-identical (B1).** A squash reorders `CREATE TABLE` vs later
`ALTER ADD COLUMN`, so `pg_dump --schema-only` will byte-differ on a *logically identical* schema
(attnum order, auto constraint-name suffixes, emission order). The gate is therefore a
**catalog-derived structural fingerprint** (§10 EA): tables as a SET of
`{column → (type, nullable, normalized-default)}`, constraints/indexes/enums/sequences/functions/
triggers/extensions by **definition, not auto-name** — diff must be empty. PLUS a **whole-DB
seed-data** compare (a fresh-migrated DB holds only seed rows): every table's rows compared by
content, ignoring volatile generated columns. See §10 EA + SPEC §3/§4.

Below the line is the ORIGINAL N3 numeric-preserve model, **retained for provenance only** —
superseded by N3.1 above; do not implement it.

**~~Version numbers become timestamps~~ (N3, superseded).** ~~Only *new* post-extraction auth
migrations use timestamps; extracted auth files keep original numbers + byte content (checksums
preserved) so deployed DBs see no change.~~ Void under N8 (no deployed DBs) + N3.1 (squash all).

**Build-DB tie-in.** App build DB = merged set (so app `query!` joining `users` verify);
`build.rs` runs `MERGED_MIGRATOR`. SDK CI provisions an **auth-only** build DB (just
`AUTH_MIGRATOR`) — `ziee-auth`'s queries only touch auth tables.

**On SDK change:** author `sdk/crates/ziee-auth/migrations/<timestamp>_x.sql` (append-only) → SDK
CI verifies against auth-only DB → app bumps the submodule → app `build.rs` re-provisions with the
merged set → app deploy `migrate run` applies it (version > last-applied).

**Three hard rules (in force AFTER the N3.1 squash baseline):** (1) **append-only / immutable** —
never edit a released migration; a CI checksum guard is **re-armed from the squash baseline commit
forward** (N8 suspended it only for the one squash — no deployed DBs then). (2) **forward-only SDK
pinning across a migration boundary** (a downgrade leaves an "applied but unknown" migration → needs
`ignore_missing`; avoid). (3) **version = `<YYYYMMDDNNNN>` monotonic** (not wall-clock seconds) for
all migrations.

**Alternative (open, if merged coordination proves painful):** a dedicated Postgres schema
(`auth.*`) with its own migration-tracking table, isolated from `public.*`; cross-schema FKs work;
cost = schema-qualification + `search_path=public,auth` for `query!` verification. Not chosen —
the merged approach is least disruptive to existing deployments.

---

## 8. Open questions needing human decision

**Resolved (human decisions):**
- **#1 Identity model → PLUGGABLE.** `ziee-framework` stays identity-agnostic — enforcement is
  generic over an injected resolver (Chunk B3); `ziee-identity` holds only abstractions/traits;
  `ziee-auth` is the **default, replaceable** impl. An app may supply its own identity.
- **#4 SDK consumption → SUBMODULE + path deps** (like ziee; no publishing infra for now).
- **#6 Sandbox → extract `ziee-sandbox` into the SDK LATER** (post-v1; both apps consume it from
  the SDK; the deferred ~1–2 wk leaf cut).
- **#12 Desktop override → BUNDLE-KEYED** (two entry bundles; each registers its overrides at boot).
- **#10 SDK ships schema → YES**, `ziee-auth` only; framework crates stay build-DB-free (§7).
- **Migration composition** → merged Migrator + module-owned squash (N3.1), `<YYYYMMDDNNNN>` monotonic versioning; migration-chain history **reconstructed, not preserved** (N8); equivalence via EA logical fingerprint + whole-DB seed (§7.1, §10).

**Lower-level questions — RESOLVED this audit pass (resolutions below):**
2. Nested-workspace mechanics (Cargo path-deps-across-workspaces; npm hoist) — validate in Chunk 0.
3. History-preservation tooling — `git filter-repo` per chunk vs one bootstrap extraction.
5. `.cargo/config.toml` ownership (`POSTGRESQL_VERSION` for the desktop harness; static-libseccomp).
7. Exact home of design-gate abstractions (`ServerConfig` core vs framework; `SyncEntityKind` crate;
   single-user strategy in `ziee-auth`).
8. Long-lived-branch rebase/freeze policy vs a moving `main`.
9. ~~Desktop trace gap~~ **RESOLVED** — desktop/dual-mode traced; §1.3 + Chunk D grounded. Refinements
   surfaced: `CORE_MODULE_BLOCKLIST` is *frontend* (a forked loader), mode-gating is *four* mechanisms,
   embedded-PG → `ziee-framework` DB bootstrap (not the harness), single-user identity → pluggable
   abstraction; worst couplings = the `Repos`/JWT/config global singletons + the `"admin"`/`is_admin`
   schema assumption.
11. `worktree_db` helper home — SDK build-support crate (shared by both apps' `build.rs`) vs copied.

**Resolutions (audit pass):**
- **#2 Nested-workspace** → prove path-deps in **Chunk 0 as a HARD go/no-go**; pre-design the
  flat-members fallback (submodule crates as ziee-workspace members, one workspace) if Cargo errors.
- **#3 History tooling** → **one up-front `git filter-repo`** of the whole platform slice into the
  SDK (history preserved once), then chunk the *refactors in-place in the SDK* — not 12× moves (§2.2).
- **#5 `.cargo/config.toml`** → stays in each app root; the SDK ships a documented template.
- **#7 Abstraction homes** → `ServerConfig`→`ziee-core`; `SyncEntityKind`→`ziee-framework`;
  single-user strategy→`ziee-auth`.
- **#8 Branch policy** → periodic rebase; the `pre-sdk-extraction` tag is the frozen E8 baseline;
  **no `main`→branch feature merges** during extraction.
- **#11 `worktree_db`** → SDK build-support crate, shared by both apps' `build.rs`.
- **N1/N5 control_mcp** → **descope C1's fresh-app criterion to v1.5.** v1 extracts only the DB-free
  *tool-dispatch* (`catalog/policy/tools`); `handlers`/`routes`/`repository`/`chat_extension` + the
  `mcp_servers` row stay app-side. A NEW app can't self-expose control until the Tier-1 `mcp`
  registry (v1.5). (Corrects the false "stateless" claim.)
- **N2 Equivalence gate** → **equivalence-preserving + re-export shims**; the byte-identical
  `types.ts` gate stays **absolute**; **spike BA's openapi diff BEFORE committing** the chunk; a
  provably-cosmetic delta needs human sign-off (no blanket declared-delta escape).
- **N3 Migrator merge** → **build-time directory composition** (runtime concat is not a supported
  sqlx API); changes `build.rs` + the `sqlx::migrate!` call site (§7.1). **→ REVISED to N3.1.**
- **N3.1 Migration reconstruction (2026-07-14, supersedes N3's "preserve numeric history").**
  The build-time-composition MECHANISM is unchanged; what changes is the source layout + naming:
  **(a) squash** the 137 app + 10 auth migrations into clean per-module baselines (unblocked by N8);
  **(b)** ALL migrations become `<timestamp>_<module>_<desc>.sql` (feature+timestamp, not just new
  ones); **(c)** migrations are **module-owned** (N7). build.rs globs `modules/*/migrations/ ∪
  sdk/crates/*/migrations/`, timestamp-sorts, composes `migrations-merged/`. Tracked by the
  **MIGRATE-squash** chunk. See DECISION_LEDGER.md for re-audit status.
- **N4 Boundary CI** → **scoped subset per boundary** (touched modules + golden diffs + dual-build);
  **full ziee suite + `gate:ui` only at the pre-merge gate** (+ nightly) (§9.4).
- **N6 De-globalize** → a **dedicated Chunk BG** (Repos/JWT/config behind traits) BEFORE B3, since
  the globals gate the whole extraction (§3).
- **N7 Module-owned migrations (2026-07-14).** Every module owns `migrations/` co-located with its
  routes/permissions/repository; the framework composes ⋃ all modules' migrations (the migration
  analog of `MODULE_ENTRIES`). Rationale: the extract-a-module-to-a-crate future — a module-crate
  must carry its own schema (as `ziee-auth` already does). A central flat dir hides ownership (root
  cause of the `27_fix_default_user_permissions` domain-perm leak). Forces one explicit owner per
  shared table (`files`, `file_chunks`, …) — a feature, not overhead.
- **N8 Pre-release squash freely (2026-07-14, human-confirmed).** No live third-party ziee Postgres
  deployments to protect → the append-only/checksum-immutability rule (N3 hard-rule #1) is
  CONSCIOUSLY SUSPENDED for the one squash, then re-established from the new baseline forward.
- **N9 Domain-seed boundary (2026-07-14).** An SDK/module migration must not seed another module's
  domain data. `ziee-auth` migrations contain ZERO permission strings other than `profile::*` / `*`;
  domain perms live in the owning module's migration. New EA grep-assertion enforces it.

**Audit corrections applied:** `AppError` count → **323**; `control_mcp` is not stateless (N1);
`all_permissions()` is `#[allow(dead_code)]`/unwired (so B3/BA *introduce* the wired registry).

---

## 9. Testing & regression strategy — proving ziee (incl. chat) is unchanged

The extraction is a **behavior-preserving MOVE**, so testing has two goals: (a) the SDK's own
tests prove the moved code works standalone; (b) **ziee's existing suite proves nothing observable
changed** — the "doesn't break chat" guarantee.

### 9.1 Two test homes
- **SDK CI** (`ziee-ai/sdk`) — the moved code's tests, run against the SDK standalone:
  - Framework crates (`ziee-core`/`ziee-identity`/`ziee-framework`) — unit tests, **DB-free**.
  - `ziee-auth` — login/session/token-rotation/RBAC integration tests against an **auth-only
    Postgres** + a minimal test host (`examples/auth-test-host` = framework + ziee-auth, per-test
    UUID DB, mirroring ziee's `tests/common/harness_inner.rs`).
  - `@ziee/framework`/`@ziee/kit` — store-kit/proxy/event-bus/sync-client unit tests (vitest).
  - `types_ts_parity` — the `emit_ts` generator golden test (fixture-spec → `types.ts` byte-identity).
  - `skeleton-server` — boots on framework-only (proves app-agnostic + auth-optional, §4).
- **ziee CI** (`ziee-ai/ziee`) — the **FULL existing suite, UNCHANGED**, against
  ziee-consuming-the-pinned-SDK: unit + integration (`--test-threads=6`) + E2E + `gate:ui`. This is
  the regression net.

### 9.2 The "doesn't break chat" guarantee (layered, checked at each chunk boundary)
Chat sits on every layer the extraction touches — module system (B2), auth/permissions (B3/BA),
sync (B5), codegen types (B6), store-kit/ApiClient (F2) — so the chat suite is the canary for
nearly every chunk. Four layers:
1. **Full ziee suite green** — incl. `chat::` integration, the real-LLM chat tests, and chat E2E.
2. **Behavior-preserving discipline** — a MOVE chunk should require **zero edits to ziee's
   behavioral assertions**. Import-path updates in test files are fine; an assertion/behavior edit
   is a **red flag** that the extraction changed behavior → stop and investigate.
3. **OpenAPI + `types.ts` golden diff (master invariant).** Snapshot `openapi/openapi.json` +
   `api-client/types.ts` before extraction; after each chunk regenerate and assert **byte-identical**.
   A changed route/type/schema surfaces immediately — this single check covers an enormous fraction
   of "did anything observable change."
4. **DB schema + seed equivalence (EA).** Capture the pre-extraction schema fingerprint + whole-DB
   seed image; after MIGRATE-squash assert the merged migrator (on a FRESH DB) yields an **identical
   logical schema fingerprint** (catalog-derived, name/order-invariant — NOT byte-identical, B1) and
   an **equivalent whole-DB seed image** (content, ignoring volatile cols). No checksum errors. (No
   real-ziee-DB-copy step — N8: no deployed DBs to protect.)

### 9.3 Extraction-specific techniques
- **Characterization tests first** for thin-coverage security-critical code (Session & Token
  Refresh, permission enforcement): add a behavioral net **before** moving it.
- **The submodule pin** means ziee CI tests the exact SDK commit — no version skew in a CI run.
- **Golden tripwires** (types, spec, schema) catch drift unit tests might miss.

### 9.4 Cadence & isolation
- **Per-commit (fast):** touched modules + the golden diffs (types/spec) + a chat smoke.
- **Per-chunk-boundary (scoped, decision N4):** touched-module tests + the golden diffs
  (openapi/types/schema) + dual clean-build. The **full ziee suite + `gate:ui`** run at the
  **pre-merge gate** (+ nightly), NOT at every boundary — the goldens still catch any observable
  change cheaply.
- **Worktree isolation:** the long-lived branch runs in its own worktree → its own `ziee_build_<key>`
  (build DB) + a **private `CARGO_TARGET_DIR`** (avoid shared-target macro cross-pollution). SDK
  auth CI uses its own Postgres service on a distinct port.
- Plugs into the `feature-lifecycle` **explicit test-enumeration** phase: every chunk's design-gate
  maps to named SDK tests + the ziee regression checks above.

---

## 10. Extraction gates & hardening — the feature-lifecycle analog

**Verdict: yes — reuse the philosophy + much of the machinery, but FLIP the core gate.** ziee's
`feature-lifecycle` gives deterministic, external, **non-self-certifiable** gates
(`lifecycle-check.mjs` + `merge-gate.mjs` + pre-push hook — Node + git, an agent can't assert-pass
them). But its machinery assumes a **single repo, a single `origin/main` base, and a "feature
build" (new behavior proven by a new test)** — none of which fit a **chunk-sequenced, two-repo,
behavior-preserving MOVE**. So we keep the *shape* (per-unit artifacts + a deterministic validator
+ convergence loops + a merge-gate) and build an **`extraction-check.mjs`** with an
extraction-shaped gate set.

### 10.1 What transfers directly (reuse from `merge-gate.mjs`, near-purpose-built)
- **C3 regen-parity** (`just openapi-regen` → empty diff on all 4 generated files) — this IS the
  master **equivalence** check for a MOVE; reused verbatim, run at every chunk boundary.
- **C1 clean-build** (`cargo clean -p … && cargo check --tests` from a fresh staging worktree) —
  catches warm-build proc-macro masking; **doubly relevant** (we move `declare_repositories!` and
  hit the known shared-target macro-pollution gotcha). Run for BOTH the SDK standalone and
  ziee-on-the-pinned-SDK.
- **C2 migration-collision** — reused for Chunk BA (merged migrator).
- **P2 merge-completeness** (every source-slice file exists in the destination tree).
- **A3** (no added skip/ignore), **A4** (no cosmetic assertion), the **blind-audit full-diff-hunk
  coverage ≥3 angles/hunk**, the **drift/fix-round convergence-to-0 loops**, and the **pre-push
  deterministic enforcement** — all carry over per chunk.

### 10.2 The core flip: "new behavior proven" → "equivalence proven"
`lifecycle-check` phase-3 enforces a bipartite **ITEM⇄TEST** map (every plan item proven by a *new*
test asserting *new* behavior). A MOVE has no new behavior, so that gate doesn't apply — and forcing
it would push you to fabricate tests or `[DESCOPED]`-approve everything. It is replaced by a
**CUT-MANIFEST⇄TRANSFORM** map + **equivalence tripwires**: extracted code must be byte-identical to
its source **modulo explicitly-declared transforms**, and nothing observable may change.

### 10.3 The per-chunk gate set (`extraction-check.mjs --chunk <id>`)
Artifacts under `.extraction/<chunk>/`: `CUT.md` (files+symbols that move), `TRANSFORMS.md` (every
non-byte-identical change + rationale — the DECISIONS analog), `LEDGER.jsonl` + `AUDIT_COVERAGE.tsv`
(blind audit), `BOUNDARY.md` (green evidence).

| ID | Asserts (deterministic) | Lifecycle analog |
|---|---|---|
| **E1** | exactly one `.extraction/<chunk>/` dir | A1 |
| **E2** | clean working tree at the boundary | A2 |
| **E3** | no diff-added `#[ignore]`/`.skip`/`.only` | A3 |
| **E4** | no cosmetic assertion **AND no edited *behavioral* assertion** on an existing ziee test (import-path edits OK; assertion-body edits fail → behavior may have changed) | A4 + NEW |
| **E5** | every file/symbol in `CUT.md` now exists in the SDK crate | P2 |
| **E6** | every file in `CUT.md` is **deleted from ziee** (no divergent duplicate — §2 single-source) | NEW |
| **E7** | every symbol changed during the move is declared in `TRANSFORMS.md`; byte-identical moves need none | NEW (equivalence-modulo-transform) |
| **E8** | **`openapi.json` + `types.ts` byte-identical** vs the `pre-sdk-extraction` baseline (schema/seed equivalence is **EA**'s job — logical fingerprint + whole-DB seed, NOT byte-identical, post-squash B1) | C3 (master gate) |
| **E9** | dual clean-build: SDK standalone **and** ziee-on-pinned-SDK | C1 (both sides) |
| **E10** | full ziee suite + `gate:ui` green at the boundary | phase8 / A7 |
| **E11** | `skeleton-server` builds framework-only (no domain/auth pull-through) | app-agnostic guard |
| **E12** | ziee branch pins an SDK commit that builds; pointer committed | NEW (cross-repo) |
| **EA** | (Chunks BA + MIGRATE-squash) merged migrator applies on a FRESH DB, no checksum errors; **logical schema fingerprint** (catalog-derived, name/order-invariant) == baseline; **whole-DB seed-data** compare == baseline (content, ignoring volatile cols); N9 auth-perm grep clean. NOT byte-identical (B1). Append-only re-armed from the squash baseline commit (N8). | C2 + §7.1 |
| **E-audit** | blind audit: full per-chunk-diff hunk coverage ≥3 angles; findings converged to 0 | phase6/7 |

### 10.4 What must be built new
- **`extraction-check.mjs`** — the chunk-phase validator (table above), keyed to `CUT.md`/
  `TRANSFORMS.md` instead of `PLAN.md`/`TESTS.md`.
- **Baseline snapshots** — a `pre-sdk-extraction` git tag on ziee + committed snapshots of
  `openapi.json`, `types.ts`, and `pg_dump --schema-only`; E8 diffs against them.
- **Cross-repo handling** — the two hardest single-repo assumptions to lift: (a) a **per-chunk base**
  (the prior chunk's boundary commit, not a moving `main`); (b) awareness of the paired SDK repo via
  the submodule (E9/E12). (`lifecycle-check`'s A1 "exactly one feature dir" + `origin/main`-only base
  are exactly what block direct reuse.)
- **Permanent boundary tests in SDK CI** — `skeleton-server` (E11) + `auth-test-host` (§9.1).

### 10.5 Enforcement
Same posture as lifecycle: **deterministic/external, not self-certifiable.** A **per-chunk-boundary
gate** (`extraction-check.mjs --chunk <id>` must exit 0 before the next chunk) + a **pre-push hook**
running `--all` on the extraction branch. The behavioral **P1/B-series** rules carry over (the
orchestrator re-runs the load-bearing gate himself; never trusts a sub-agent's self-report). The
**final** merge of the extraction branch → the existing `merge-gate.mjs` (C1/C2/C3/P2/C5) applies
as-is.

### 10.6 The per-chunk quality loop (drift / audit / test must converge)

§10.3's E-gates are the *boundary* checks; **inside** each chunk runs a mini-lifecycle whose
drift/audit/test phases must **converge or pass** before the boundary — the direct analog of
feature-lifecycle phases 4–8, gated by `extraction-check.mjs --chunk <id> --phase P` (phase P must
exit 0 before P+1; the whole chunk must pass before the next chunk starts):

| Phase | Artifact(s) | Convergence / coverage gate | Lifecycle analog |
|---|---|---|---|
| **C-1 plan** | `CUT.md` (files+symbols to move), `TRANSFORMS.md` (every non-byte-identical change + rationale + the design-gate decision) | manifest well-formed; every transform carries a rationale; **zero `TBD/TODO/ASK`** | phase 1/4 |
| **C-2 move + drift** | `DRIFT-N.md` | **unresolved drifts → 0**: every moved file ∈ `CUT.md`; every changed symbol ∈ `TRANSFORMS.md`; no ziee ref still points at the old location; equivalence tripwires (openapi/types) not red | phase 5 |
| **C-3 blind audit** | `LEDGER.jsonl` + `AUDIT_COVERAGE.tsv` | ≥ `ANGLE_MIN` angles incl. an **equivalence** angle ("behavior beyond declared transforms?") + a **security** angle for auth/permission/control chunks; **every diff hunk ≥3 angles** (reconciled against `git diff <prev-boundary>...HEAD`) | phase 6 |
| **C-4 fix loop** | `FIX_ROUND-N.md` | **confirmed findings → 0** | phase 7 |
| **C-5 boundary** | `BOUNDARY.md` | E1–E12 (+EA) all green; the chunk's **ported tests enumerated + PASS**; **ziee suite + `gate:ui` green**; golden tripwires **byte-identical** | phase 8 |

**How "test" is gated for a MOVE.** No new behavior to prove, so enumeration flips to
**coverage-preservation**: `TESTS-MOVED.md` lists, per chunk, every ziee test that covered the moved
code and where it now runs (ported to the SDK, or still green in ziee). The gate asserts (a) each
listed test **PASSes**; (b) an **A5-style shrink-guard** — no covering test may be dropped or
renumbered vs the prior committed manifest; (c) **E4** — no behavioral assertion on a retained ziee
test was edited; (d) the chunk's **equivalence tests** (golden openapi/types/schema diffs) exist and pass.

**Machine-enforced vs. judgment.** The convergence counts (`Unresolved drifts: 0`, `confirmed
findings: 0`), the hunk-coverage reconciliation, the pass-lines, and the golden/build re-runs are
**deterministic** — `extraction-check.mjs` re-runs the tripwires itself and reads the artifact
counts, so a chunk can't advance on a self-report. Whether the audit *found the right things* stays
the behavioral **P1 rule** (the orchestrator re-runs the load-bearing gate and reviews the diff
himself) — the same honest-limit posture as feature-lifecycle.

---

## 11. SDK capability catalog & roadmap

The SDK = mandatory **core** (framework + identity + auth) + an **opt-in catalog of capability
crates**. **`control_mcp`'s tool-dispatch core is in v1** (Chunk C1); full **fresh-app self-exposure
is v1.5** (needs the Tier-1 `mcp` registry — decision N5). The rest extract **incrementally, each under the §10 gate discipline, when an app needs
it** (†-marked are schema-bound → follow the `ziee-auth` merged-migrator pattern, §7.1).

- **v1 (core + control):** `ziee-core`, `ziee-identity`, `ziee-framework`, `ziee-auth`,
  **`ziee-control-mcp`**, `@ziee/framework`, `@ziee/kit`, desktop harness.
- **Tier 1 — agent/tool (next):** `mcp` infra (built-in registry + tool-call history †),
  `js_tool`/run_js, `tool_result_mcp`, `elicitation_mcp`, `skill`/`skill_mcp`.
- **Tier 2 — app infra:** `scheduler` †, `notification` †, `file` store †, `server_update`,
  `health`, `hardware`, `onboarding` (framework; steps app-provided).
- **Tier 3 — AI/RAG packs:** `file_rag`+`knowledge_base` †, `memory` †, `llm_provider`+`llm_model`+
  `llm_local_runtime` †, `web_search`, `voice`, `summarization`.
- **Tier 4 — science packs (CytoAnalyst-relevant):** `lit_search`, `bio_mcp`, `citations` †.
- **Stays ziee-the-app:** `chat`, `assistant`, `assistant_core_memory`, `project` (`hub` borderline).

`code_sandbox` (decision #6) is a Tier-2/compute pack extracted post-v1. Extraction order is driven
by **app need**, not this list's order.

---

*End of plan. No code written or moved; no repo created; `feature-lifecycle` not started.
Awaiting human approval / decisions.*
