# SDK Extraction — Phase-1 Execution Plan

The first executable increment: **stand up the tooling + repo plumbing, then extract the backend
foundation** (`ziee-core`, `ziee-identity` abstractions, the module system + Config split) and prove
the boundary with the skeleton app. Pairs with `SDK_EXTRACTION_PLAN.md` (chunks, gates) +
`EXTRACTION_CHECK_SPEC.md` (the validator). **Still a plan — no code/repo yet.**

**Phase-1 scope:** Tooling → Chunk 0 → **B1** (ziee-core) → **B1b** (ziee-identity) → **B2** (module
system + Config split) → **Skeleton milestone**. Stops before B3/permissions. Chosen because it's the
mechanically-clean spine + the first real design gate (Config split) + the app-agnostic proof, with
zero schema and zero auth risk.

---

## 0. Prerequisites (tooling — must exist before any chunk is gated)

The gate can't gate itself into existence, so Phase-1 starts by building the harness:

- **P0.1 Repos + branch.** Create `ziee-ai/sdk` (empty). In ziee, create the long-lived branch
  `feat/sdk-extraction` **in its own worktree** with a **private `CARGO_TARGET_DIR`** (avoid the
  shared-target macro-pollution gotcha). Tag `pre-sdk-extraction` on ziee `main`.
- **P0.2 Baseline snapshot.** Run `extraction-check.mjs --baseline snapshot` (build it per
  `EXTRACTION_CHECK_SPEC.md` §4): capture `openapi.{ui,desktop}.json`, `types.{ui,desktop}.ts`,
  `schema.sql`. These are the immutable equivalence anchors.
- **P0.3 `extraction-check.mjs`.** Implement the validator (spec §1–§6). Its own selftest:
  synthetic pass/fail `.extraction/` fixtures (mirror `.claude/lifecycle/selftest.sh`).
- **P0.4 Pre-push hook + `just` helpers.** `just sync-sdk`, `sdk-status`, `sdk-checkout`; hook runs
  `extraction-check.mjs --all` on the extraction branch (§5, main plan).
- **P0.5 genericization-caveat policy — RESOLVED (N2): equivalence-preserving + re-export shims +
  spike.** The byte-identical golden gate stays absolute; refactors keep the serialized schema
  identical via thin ziee shims. Applies from B2 on (Config split is the first refactor that could
  move the gate).
- **P0.6 de-globalize is Chunk BG** (N6) — the `Repos`/JWT/config singletons are de-globalized as a
  dedicated chunk before B3 (Phase-2 start); Phase-1's B1/B1b/B2 avoid depending on new genericity
  of those globals.

**DoD:** validator selftest green; baseline captured; hook installed; empty SDK submodule present.

---

## 1. Chunk 0 — SDK skeleton + wiring bootstrap

- **CUT:** nothing moves. Create empty crates `ziee-core`, `ziee-identity`, `ziee-framework`,
  `ziee-auth`, `ziee-control-mcp` (lib stubs) + package stubs `@ziee/framework`, `@ziee/kit`;
  add the submodule at `sdk/`; wire Cargo **path-deps** + npm **workspaces**; add
  `sdk/examples/skeleton-server` (empty).
- **Design-gate (validate the riskiest plumbing assumption):** confirm the **nested Cargo
  workspace** (ziee path-deps into `sdk/crates/*` which has its OWN `sdk/Cargo.toml` workspace)
  actually resolves — build a trivial call from ziee into an SDK stub. Same for **npm** hoisting.
  *(If nested workspaces error, fall back to the decision in §8 #2 — this is the go/no-go for the
  whole path-dep model, so it's Chunk-0's whole point.)*
- **Gate (C-5 subset):** E9 (ziee-on-empty-SDK builds), E10 (ziee suite unchanged), E12 (submodule
  pinned + builds), E8 (golden identical — nothing moved yet).
- **DoD:** ziee compiles + full suite green with the empty SDK linked; `types.ts` byte-identical.

---

## 2. Chunk B1 — `ziee-core`

- **CUT:** `common/type.rs` (`AppError`/`ApiResult`), `common/macros.rs` (+ `sse_event_enum!`),
  `core/app_state.rs` globals. **Symbols:** `AppError`, `ApiResult`, macro exports, app-state
  globals. ~311-file import rewrite `crate::common::AppError → ziee_core::AppError` (scripted).
- **TRANSFORMS:** T-1 app-state default app-name made configurable (was `~/.ziee`). Everything else
  byte-identical (pure move). Define `ServerConfig` **struct** here (empty target; the *split* is B2).
- **Steps:** move files+history into `sdk/crates/ziee-core`; scripted import rewrite in ziee;
  port `common` unit tests into the crate; delete from ziee; wire path dep; bump submodule.
- **Gate (full C-1..C-5):** E5/E6/E7 (moved+deleted+transform-declared), DRIFT→0, audit≥3angles,
  FIX→0, E8 golden identical (AppError isn't in the API surface → must stay byte-identical), E9/E10.
- **DoD:** ziee compiles on `ziee_core::AppError`; suite green; golden identical.

---

## 3. Chunk B1b — `ziee-identity` (abstractions)

- **CUT:** the identity **traits** — a `Principal`/identity trait, `PermissionCheck`/`PermissionList`
  (`permissions/types.rs`) + RBAC eval (`permissions/checker.rs:38-52` wildcard/hierarchical), a
  JWT-verify interface. **No concrete `User`/`Group` tables/types** (those stay in ziee's `user`/
  `auth` modules for now; they'll implement the traits).
- **TRANSFORMS:** T-1 extract the `PermissionCheck`/`PermissionList` traits + `check_permissions_array`
  logic into trait-generic form; T-2 define a `Principal` trait the concrete `User` implements; T-3
  a `VerifyJwt` interface (the concrete `JwtService` stays app-side, implements it). **Build-DB-free**
  — no queries move (user loading stays behind the trait, in ziee).
- **Gate:** full C-1..C-5. Watch E8: if the permission/JWT types appear in the OpenAPI spec,
  trait-genericizing must NOT rename the serialized schema (apply the §0.5 policy).
- **DoD:** ziee's `user`/`auth` implement `ziee_identity` traits; permission checks pass; golden per §0.5.

---

## 4. Chunk B2 — module system + Config split

- **CUT:** `module_api/*` (`AppModule`/`ModuleContext`/`MODULE_ENTRIES`), `core/app_builder.rs`
  (discovery + `build_api_router` + CORS/rate-limit). **Symbols:** `AppModule`, `ModuleContext`,
  `ModuleEntry`, `MODULE_ENTRIES`, `create_modules`, `build_api_router`.
- **TRANSFORMS (the first real design gate):** T-1 **Config split** — `ModuleContext` carries only
  `ServerConfig` (postgresql/server/jwt/logging); ziee's monolithic `Config` (`config.rs:5-45`)
  **composes** `ServerConfig` + its domain sub-configs and implements the framework context trait.
  T-2 `ModuleContext` generic over / holding `ServerConfig` instead of `crate::core::config::Config`.
- **Risk to the golden gate:** the Config split reshapes config types. If any config type is in the
  OpenAPI surface (e.g. an admin `GET /config` DTO), `types.ts` moves → §0.5 policy decides
  (equivalence-preserving vs declared-delta). **This is why §0.5 blocks B2.**
- **Gate:** full C-1..C-5; E9 dual-build especially (module registration wiring); E8 per §0.5.
- **DoD:** ziee registers its modules against the moved `MODULE_ENTRIES`; boots; suite green.

---

## 5. Skeleton milestone (the app-agnostic proof)

- Build `sdk/examples/skeleton-server`: depends on **only** `ziee-core` + `ziee-framework`; registers
  one module + `GET /api/ping`; emits its own `types.ts` via the SDK generator (once B6 lands — until
  then, a backend-only skeleton with no codegen); boots.
- **Gate E11:** compiles + boots with **zero** ziee domain/auth/chat pull-through. If it can't, the
  framework boundary leaked → **stop; fix before B3**.
- Wire `skeleton-server` into SDK CI **permanently** (regression guard).
- **DoD:** skeleton green in CI; proves B1+B1b+B2 produced an app-agnostic framework.

---

## 6. Sequencing & dependencies

```
P0 tooling ─▶ Chunk0 ─▶ B1 ─▶ B1b ─▶ B2 ─▶ Skeleton
                                  │
                          (§0.5 policy gates B2)
```
Frontend F1 (`@ziee/kit`) MAY run in parallel after Chunk 0 (independent build graph) but is out of
Phase-1's critical path. Each arrow is a **green chunk boundary** (`extraction-check.mjs --chunk <id>`
exits 0) before the next starts.

---

## 7. Phase-1 Definition of Done
- Tooling live (validator selftest green, baseline captured, hook installed).
- `ziee-core` + `ziee-identity` + the module system extracted; ziee consumes them via path-deps.
- Config split landed; ziee boots + full suite + `gate:ui` green; golden gate satisfied per §0.5.
- `skeleton-server` proves the framework is app-agnostic, wired into SDK CI.
- Every chunk boundary passed `extraction-check.mjs`; `feat/sdk-extraction` is green and rebased on
  a recent `main`.

**Explicitly NOT in Phase-1:** B3 (permissions), B4 (Repos macro), B5 (sync/SyncEntity), B6
(emit_ts), BA (auth — schema/migrations), C1 (control_mcp), frontend F2, desktop D. Those are Phase-2+.

---

## 8. Human decisions — RESOLVED (audit pass)
1. Genericization policy → **equivalence-preserving + shims + spike** (N2).
2. Nested-workspace → **prove in Chunk 0 (hard go/no-go)**; flat-members fallback pre-designed (#2).
3. History → **one up-front `filter-repo`**, refactor in-place (#3).
4. `worktree_db` → **SDK build-support crate** (#11).
All Phase-1 blockers resolved. The full consolidated decision log is `SDK_EXTRACTION_PLAN.md` §8.
