# SDK Extraction — Decision & Drift Ledger (standing, cross-chunk)

The program-level spine the per-chunk `DRIFT-N.md` files don't provide. Per-chunk drift converges
to 0 *within* a chunk and freezes at its boundary; this ledger tracks **load-bearing decisions
across the whole extraction**, records when one is **revised**, names the chunks each touches, and
carries a **re-audit status** so a revised decision can't silently leave an already-gated chunk
non-compliant. Analog of the lifecycle skill's `DECISIONS.md` + `HUMAN_FEEDBACK.md`, applied at the
program level.

**Gate (to wire into `extraction-check.mjs --all`):** fail if any decision is `status: revised`
with `reaudit: pending` and no tracking chunk in `.extraction/ORDER`; and every chunk listed under a
revised decision's `affects` must have a boundary commit **newer** than the revision date (else it's
stale against the new decision). Grammar is regex-parseable (`- status:`, `- reaudit:`, `- affects:`).

Status vocab: `active` | `revised` | `superseded`. Reaudit vocab: `clean` | `pending` | `n/a`(process).

---

## Decisions

### N1 — Identity model: PLUGGABLE
- statement: framework is identity-agnostic; enforcement is generic over an injected resolver;
  `ziee-identity` holds only traits; `ziee-auth` is the default replaceable impl.
- status: active
- decided: 2026-07-12
- affects: B1b, B3, BA
- reaudit: pending

### N2 — Equivalence gate: equivalence-preserving + re-export shims
- statement: byte-identical `types.ts` gate stays ABSOLUTE; refactors keep serialized schema
  identical via ziee shims; spike openapi diff before committing; cosmetic delta needs human sign-off.
- status: active
- decided: 2026-07-12
- affects: B2, B3, B5, B6, BA
- reaudit: pending

### N3 — Migration composition: build-time directory composition  → **REVISED (N3.1)**
- statement (original): runtime Migrator concat is unsupported → app composes ONE migration dir at
  build time (auth ∪ app, version-sorted) and `sqlx::migrate!` over it. Moved historical migrations
  KEEP original numeric versions + byte content (checksum-safe for deployed DBs); only NEW migrations
  use timestamps.
- status: revised
- decided: 2026-07-12
- **revised: 2026-07-14 → N3.1.** Supersedes the "preserve numeric history" clause. Now: **squash**
  the full 137+10 history into clean per-module baselines; **ALL** migrations become
  `<timestamp>_<module>_<desc>.sql` (not just new ones); migrations are **module-owned** (see N7).
  Build-time composition mechanism is UNCHANGED (widen its source globs). Unblocked by N8
  (pre-release, no deployed DBs to protect → checksum-immutability suspended for the one squash).
- affects: BA (auth carve-out), every migration-bearing chunk, **MIGRATE-squash** (the tracking chunk)
- reaudit: **clean (2026-07-14)** — MIGRATE-squash landed: 147 numeric migrations squashed into 91
  module-owned `<YYYYMMDDNNNN>_<module>_<desc>.sql` baselines; build.rs globs the module dirs;
  EA-schema fingerprint IDENTICAL + EA-seed EQUIVALENT + N9 grep 0 + 3× cargo check green + golden
  byte/canonical-identical both surfaces. BA is re-gated by these EA anchors.

### N4 — Boundary CI: scoped subset per boundary
- statement: per-boundary = touched-module tests + golden diffs + dual clean-build; full ziee suite
  + `gate:ui` only at the pre-merge gate (+ nightly).
- status: active
- decided: 2026-07-12
- affects: all
- reaudit: n/a (process)

### N5 — control_mcp: descope C1 to tool-dispatch only; fresh-app exposure v1.5
- statement: v1 extracts DB-free tool-dispatch (catalog/policy/tools); handlers/routes/repository/
  chat_extension + the `mcp_servers` row stay app-side. Self-expose needs the Tier-1 `mcp` registry (v1.5).
- status: active
- decided: 2026-07-13
- affects: C1
- reaudit: pending

### N6 — De-globalize: dedicated Chunk BG before B3
- statement: Repos/JWT/config singletons de-globalized behind traits as a dedicated chunk before B3.
- status: active
- decided: 2026-07-12
- affects: BG, BG-2, BG-3, B3, D-full
- reaudit: pending

### N7 — Migrations are MODULE-OWNED (new, this session)
- statement: every module owns `migrations/` (co-located with its routes/permissions/repository),
  and the framework composes ⋃ all modules' migrations. Mirrors the `MODULE_ENTRIES` registry
  pattern. Rationale: the extract-a-module-to-a-crate future — a module-crate must carry its own
  schema (as `ziee-auth` already does). A central flat dir hides ownership (root cause of the
  `27_fix_default_user_permissions` leak). build.rs globs `modules/*/migrations/ ∪
  sdk/crates/*/migrations/`, timestamp-sorted.
- status: active
- decided: 2026-07-14
- affects: MIGRATE-squash, all future module extractions
- reaudit: clean (2026-07-14 — landed with MIGRATE-squash; migrations now module-owned + build.rs globs)

### N8 — Pre-release: squash freely (no deployed DBs)
- statement: no live third-party ziee Postgres deployments to protect → the checksum-immutability /
  append-only rule (N3 hard-rule #1) is CONSCIOUSLY SUSPENDED for the one squash, then re-established
  from the new baseline forward. Human-confirmed 2026-07-14.
- status: active
- decided: 2026-07-14
- affects: MIGRATE-squash
- reaudit: n/a

### N10 — Auth HTTP surface moves to the SDK (complete the auth extraction, v1)
- statement: BA extracted the auth ENGINE but under-extracted the SURFACE — the aide REST handlers
  (`register`/`login`/`refresh`/`logout`, the full OAuth flow incl. `oauth_callback_post`,
  `jwt_extractor`, `session_settings`, providers CRUD) are generic auth MECHANISMS, not ziee-domain,
  so they move to the SDK as a **mountable `ziee-auth-routes` bundle** (feature/submodule of
  ziee-auth). ziee's `auth` module shrinks to "mount SDK routes + supply provider config/branding".
  Only CONFIG (enabled providers, client secrets, redirect URLs, branding) stays app-side.
- status: active
- decided: 2026-07-14 (human: "some should be in the sdk, e.g. oauth_callback_post" → chose "in v1")
- affects: ziee-auth-routes (new chunk), ziee's auth module
- reaudit: **clean (2026-07-14)** — chunk `ziee-auth-routes` landed. The auth HTTP
  surface (handlers/routes/jwt_extractor/session_settings + auth-domain permissions +
  the Profile permission family) moved to `ziee-auth/src/auth/http` + `auth/permissions.rs`
  + `user/permissions.rs` as a mountable, resolver-generic routes bundle
  (`auth_routes<R>` / `auth_admin_routes<R>`, R: IdentityResolver<User=User,Group=Group>,
  feature `routes` default-on). ziee's `auth` module shrank to ONE file (`mod.rs`, thin
  consumer: mounts SDK routes with `ZieeIdentityResolver` + module-path shims). All 3
  design-gates resolved (see `.extraction/ziee-auth-routes/TRANSFORMS.md`): (1) golden
  byte/canonical-identical on BOTH surfaces; (2) emits only crate-local
  AuthSyncEntity/AuthSyncAction via the injected AuthSyncSink — zero ziee-domain
  SyncEntity in the SDK; (3) redirect derives from headers+trust_forwarded_headers
  static, providers from the auth_providers table — no ziee Config/Repos global.
  Gates: cargo check ziee=0 / ziee-desktop=0 / sdk workspace=0 / skeleton framework-only=0;
  auth::admin_providers_test 10/10 green on the moved surface (full suite blocked only by
  a PRE-EXISTING MIGRATE-squash harness gap, untouched here).
- design-gates: (1) **N2 byte-identical OpenAPI** — handlers mounted at same paths must keep
  operationIds + schema names identical (preserve type names via re-export shims; spike openapi diff
  BEFORE committing). (2) **sync emission** — handlers publish auth entities (User/Group/Session/
  SessionSettings); emit via `ziee_framework::sync` (Audience/publish already in framework) over the
  `SyncEntityKind` trait (B5) with auth-owned entities, NOT ziee's `modules::sync`. (3) **config
  injection** — enabled providers + redirect base URL injected via a config trait/struct, not read
  from ziee globals.

### N9 — Domain-seed boundary (new; the leak fix + a gate)
- statement: an SDK/module migration must NOT seed another module's domain data. Concretely:
  `ziee-auth` migrations contain ZERO permission strings other than `profile::*` / `*` — domain
  perms (`chat::`,`branches::`,`assistants::`,`mcp_servers::`,`hub::`,`files::`,`conversations::`,…)
  live in the owning module's migration. New EA assertion greps for violations.
- status: active
- decided: 2026-07-14
- affects: BA (re-audit), MIGRATE-squash
- reaudit: clean (2026-07-14 — auth migrations grep-clean: 0 non-profile/non-* perm strings)

---

## Earlier resolutions (stable; from §8 audit pass)
- #1 identity→pluggable(=N1) · #2 nested-workspace→prove-in-Chunk0(done, retired) ·
  #3 history→one filter-repo · #4 consumption→submodule+path-deps · #5 `.cargo/config`→app-root+SDK template ·
  #6 sandbox→extract LATER(post-v1) · #7 abstraction homes(ServerConfig→core, SyncEntityKind→framework,
  single-user→ziee-auth) · #8 branch→periodic-rebase, no main→branch merges · #10 SDK ships schema→auth only,
  framework build-DB-free · #11 worktree_db→SDK build-support crate · #12 desktop override→bundle-keyed.

---

## Human-feedback ledger (Phase-9 analog)
- **FB-1 [2026-07-14] domain-perm leak in auth.** "`27_fix_default_user_permissions` doesn't make
  sense there; why does core know about chat/branches?" → Resolution: N9 + N7 (module-owned) + split
  the mixed migrations. **generalizable: yes** — "a crate/module migration must not seed another
  module's domain data; gate it" (folded into EA as N9's grep assertion).
- **FB-2 [2026-07-14] feature+timestamp, module-owned migrations for extract-to-crate.**
  "reconstruct migrations labeled by feature+timestamp instead of number… truly a meta framework."
  → N3.1 + N7. **generalizable: yes** — migration authoring convention doc + the module-owned glob.
- **FB-3 [2026-07-14] standing drift ledger.** "maintain a drift ledger like the lifecycle skills."
  → THIS artifact + the `--all` gate above. **generalizable: yes** — program-level decision ledger
  is now a required extraction artifact.

---

## Re-audit tracker (impl-vs-plan, driven by the revisions above)
Trigger: N3→N3.1 revision + the concern that a revised load-bearing decision may reveal latent drift
in already-gated chunks. Scope = every completed chunk checked against its decisions + `.extraction`
artifacts + actual code.

| Chunk | Primary decisions | Re-audit status |
|---|---|---|
| chunk0 | #2,#4 | pending |
| B1 | #7 (ServerConfig→core) | pending |
| B1b | N1 | pending |
| B2 | N2, #7 | pending |
| B3 | N1, N2, N5-adjacent | pending |
| B4 | N2 | pending |
| B5 | N2, #7 (SyncEntityKind) | pending |
| B6 | N2 (emit_ts parity) | pending |
| F1 | — | pending |
| C1 | N5 | pending |
| BG / BG-2 / BG-3 | N6, #7 (embedded-PG→framework) | pending |
| F2 | — | pending |
| **BA** | N1, N2, N3→**N3.1**, N9, #10 | **pending (highest priority — the migration+seed locus)** |
| D / D-full | N6, #12 | pending (D-full mid-flight) |

**Sequence:** update plan (N3.1/N7/N8/N9 + EA) → audit the updated plan → run this impl-vs-plan
re-audit → implement MIGRATE-squash → re-gate BA + MIGRATE-squash → clear the pending rows.

---

## Plan-audit convergence (2026-07-14) — findings → resolutions
Three parallel read-only audits ran against the revised plan + the extracted code. Results:
- **Boundary-purity (impl):** 5/5 boundaries clean (N1 trait-only, #7 homes, #10 build-DB-free,
  N5 descope, E11 skeleton). **MEDIUM:** `ziee-control-mcp/src/tools.rs` descriptors ship the
  literal "ziee" app-name + domain nouns (assistants/users) to the LLM → same leak CLASS as N9.
  **Resolution → N9 generalized:** app-agnostic SDK crates must not hard-code the app name or
  domain nouns in shipped strings; control-mcp descriptors get templated app-name + neutral
  examples. **LOW×3:** test-fixture domain coupling (identity/core/policy) — opportunistic fix.
- **Equivalence + de-globalize (impl):** N2 + N6 genuinely clean (shims real, no schema rename,
  types.ts byte-identical both surfaces re-confirmed, boot-path threaded not disguised-global).
  **LOW:** `BootHandle.pool` sourced from `ziee::Repos.pool()` inside the *app-side* `ZieeServerBoot`
  (allowed); optional future tightening (return pool from `start_server_with_routes`).
- **Plan-audit:** **BLOCKER B1** — byte-identical `pg_dump --schema-only` is the WRONG equivalence
  relation for a squash (attnum order, auto constraint-name suffixes, emission order diverge on
  logically-identical schema). **Resolution:** EA switches to a **logical, catalog-derived schema
  fingerprint** (name/order-invariant: columns as a set of {name,type,nullable,norm-default};
  constraints/indexes by definition not name; enums/sequences/functions/triggers/extensions by
  definition) — diff must be empty; validator RE-RUNS on both DBs. **H1** stale §7.1/§9.2(4)/§10.3
  → rewrite to N3.1. **H2/H3** seed anchor unimplemented + curated → implement in SPEC §4/§3 as a
  **whole-DB** data image compare (a fresh-migrated DB has ONLY seed data → dump all rows all
  tables, compare per-table by content excluding volatile generated-UUID/timestamp columns;
  FK-referenced seed rows keep literal ids). **H4** ownership map defined (auth tables→ziee-auth;
  files→files; file_chunks/file_index_state→file_rag; join tables→parent module;
  `CREATE EXTENSION vector`→framework/core bootstrap-first; domain perm-grants→feature-owning
  module). **H5** ordering = date-prefixed monotonic counter `YYYYMMDDNNNN` (unique + FK-topo-valid,
  gated), not wall-clock seconds. **H6** ledger grammar normalized + the `--all` decision-gate is a
  documented rule for now (not yet machine-parsed). **M1** N8 append-only re-established from the
  squash baseline commit forward (checksum guard re-armed). **M2** MIGRATE-squash is a
  reconstruction, EXEMPT from the move-shaped C-2 E5/E6/E7 (it doesn't move symbols) — gated by EA
  equivalence + N9 instead. **§2.4 invariant** carve-out: git code-history preserved; migration-chain
  history deliberately squashed (the two disambiguated).
- reaudit status: N1/N2/N5/N6/#7/#10/E11 → **clean** (this pass). N3.1/N7/N9 → pending (land with
  MIGRATE-squash + the control-mcp/test-fixture domain-neutralization sweep). BA → re-gated by
  MIGRATE-squash.

**Convergence round 2 (2026-07-14) — re-audit of the converged plan.** 3 HIGH + 2 MEDIUM, all closed
on paper (no BLOCKER): **(H1-schema)** fingerprint now includes `is_generated`+`generation_expression`
(ziee has 3 GENERATED cols — `content_tsv`×2 + `user_memories`), `pg_get_indexdef` (opclass
`halfvec_cosine_ops` + predicate), `pg_get_constraintdef` (FK ON UPDATE/DELETE/DEFERRABLE, CHECK).
**(H1-seed)** compare algorithm specified: business-key row keying (baseline uses random uuids),
FK-via-business-key join, drop volatile cols, element-sort set-arrays (`groups.permissions TEXT[]`).
**(H1-residual)** stale BA gate + §8 "preserve version/checksum / existing history preserved"
reconciled to N3.1. **(MEDIUM)** index/FK fingerprinting folded in; ledger grammar normalized to
standalone field lines (all 9 decisions uniform → a future `--all` regex won't skip any). **Verdict:
GREEN LIGHT for MIGRATE-squash implementation.**
