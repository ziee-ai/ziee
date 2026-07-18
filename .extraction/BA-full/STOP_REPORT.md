# Chunk BA-full — STOP-and-report (residual auth/user couplings beyond BG's 6 globals)

**Verdict: NOT SAFE to force the full move yet.** BG (commit `e0a7503b9`)
severed the **six named app-globals** the original BA STOP_REPORT listed. That is
real and verified. But the `modules/auth/*` + `modules/user/*` tree still holds
**four additional crate-crossing couplings** that BG's exit grep did **not**
cover (it grepped only for those six specific symbols). Two of them are
**app-wide infra relocations BG explicitly deferred**, one is an **unplanned
architecture decision**, one is an in-move transform. Until the two infra ones
are resolved, `ziee-auth` cannot compile against only
`ziee-core`/`ziee-framework`/`ziee-identity` without either (a) dragging
app-wide infrastructure into the SDK on an improvised call, or (b) breaking the
build. This is the task's sanctioned "STOP rather than force a broken change,"
and it mirrors the exact discipline the original BA→BG STOP established (sever
in-ziee first, THEN move).

**No code was moved; no SDK commit; `.extraction/ORDER` unchanged; working tree
clean (only this report added).**

---

## What IS ready (de-risked, confirmed this pass)

- **BA-spike landed** — `User`/`Group` concrete wire types live in `ziee-auth`
  (`sdk` `844278a`), golden-clean; schemars keys types by short ident so the
  crate boundary leaves the OpenAPI schema names + every `$ref` byte-stable.
- **The 6 globals ARE gone** from `modules/auth/*` + `modules/user/*`:
  `crate::core::Repos`, `sync::publish[_session_to_users]` (the FUNCTION),
  `secrets::storage_key`, `AppEvent`/`EventBus`, `core::config::JwtConfig`, and
  the direct `url_validator` name — replaced by the injected `AuthContext` +
  `AuthEventSink`/`AuthSyncSink` seams + threaded `&PgPool`/`storage_key` params.
- **Build DB available** — `ziee-postgres-build-1` up on `:54321` (pg18); an
  auth-only build DB can be a sibling database `ziee_auth_build_<key>` on the
  same cluster (no new container needed). Free ports also present.
- **`build.rs` already uses a runtime `Migrator::new(dir)`** over a list of dirs
  with `set_ignore_missing(true)` (`build.rs:158-205`) — the N3 build-time
  directory-composition target is a small edit to an existing loop, not a
  rewrite. Runtime site is `core/database/mod.rs:318` (`sqlx::migrate!("./migrations")`).
- **Migration composition is safe at the file level.** `initial_schema.sql`
  (mig 1) is **purely auth** (users/groups/user_groups/auth_providers/
  user_auth_links/oauth_sessions) — no non-auth DDL entangled, so it moves whole
  with checksum preserved. The app-domain `grant_*_permissions` migrations
  (35/39/54/61/85/96/98/101/104/107/126/134/142/147/152) only `UPDATE groups`
  and stay app-side; version-sorted, the merged set reproduces ziee's exact
  `_sqlx_migrations` history → deployed DBs unaffected.

---

## The FOUR residual couplings (evidence-backed) — why the move can't compile yet

### C1 — `common::secret` (at-rest secret crypto) — UNPLANNED HOME DECISION
`modules/auth/providers/repository.rs:11`
`use crate::common::secret::{encrypt_secret, resolve_optional_secret};`

- **Build-DB-free** — uses runtime `sqlx::query_as("SELECT pgp_sym_encrypt(...)")`,
  NOT the `query!` macro (`common/secret.rs:93,118`). So it is a domain-free
  crypto helper, not schema-bound. `encrypt_secret(pool, plaintext, storage_key)`
  already takes the (BG-threaded) key as a param.
- **App-wide** — used by **11 files** (llm_provider ×3, mcp ×2, web_search,
  lit_search, llm_repository, llm_local_runtime, auth). Relocating it forces a
  decision on its SDK home (`ziee-framework`? `ziee-core`? a new `ziee-crypto`?)
  and re-points all 11 via a shim. **The plan (§1–§7) never mentions
  `common::secret`** — this is an unplanned architecture decision, not a BA task
  item. BG's grep (`secrets::storage_key`) did not catch it (different path,
  `common::secret::encrypt_secret`).
- Injection alternative (async-fn `SecretCodec` trait on `AuthContext`) is
  possible but adds a runtime seam for what is really a pure library and is
  security-critical.

### C2 — `url_validator` (SSRF) — BG EXPLICITLY DEFERRED THIS
`modules/auth/providers/{oauth2,apple}.rs` name `crate::core::outbound::{OutboundUrlPolicy,build_validated_client,validate_outbound_url}` (12 sites), where `core/outbound.rs` is BG's app-side re-export of `crate::utils::url_validator`.

- **BG's own BOUNDARY.md + TRANSFORMS Decision 5 state**: url_validator is
  "domain-free framework infra whose true home is `ziee-framework`"; the move was
  deferred because BG couldn't touch the submodule, and "**when `url_validator`
  lands in `ziee-framework`, `ziee-auth` retargets the single `core::outbound`
  import.**" That move has NOT happened. So BA-full is gated on it.
- **Build-DB-free**, domain-free (608 lines, `utils/url_validator.rs`), used by
  **18 files** app-wide → move to `ziee-framework` + shim `crate::utils::url_validator`.
- BG explicitly warns injection into `oauth2.rs` is "**risky, non-equivalence-
  trivial**" (its outbound path is a process-global `OnceLock<reqwest::Client>`
  fed to `openidconnect` as fn-pointers — no per-request handle to thread a trait
  through). So the framework-move is the sanctioned path, not injection.

### C3 — `delete_user` cascade into APP modules (skill/file/hub)
`modules/user/handlers/user.rs:513,521,551,568` —
`crate::modules::skill::SkillRepository`, `crate::modules::file::{FileRepository,
storage::manager::get_file_storage}`, `crate::core::get_app_data_dir()` +
`<app_data>/{skills,workflows}/<uid>` cleanup.

- Pre-delete collection of skill-bundle dirs / file-blob ids is interleaved with
  the cascade DELETE (FKs are `ON DELETE CASCADE`), so it **cannot** be moved to a
  post-delete `UserEvent::Deleted` subscriber.
- **Resolution options:** (a) keep the user *admin handlers* app-side for this
  chunk (they `use ziee_auth::{AuthContext, UserRepository, User, Group}`; the
  repos/models/permissions/events/types still move) — cleanest, no new seam; or
  (b) a new `AuthUserCleanup` collect-before/cleanup-after seam on `AuthContext`.
  Either is fine, but (a) means ziee-auth does NOT own user *admin CRUD* in v1.

### C4 — `AuthSyncSink` trait names the concrete app `SyncEntity`/`SyncAction`
`modules/auth/context.rs:25,45-52` — the trait signature is
`publish(entity: SyncEntity, action: SyncAction, id, audience: Audience, origin)`.

- `Audience` is already `ziee_framework::sync::Audience` (OK to name). But
  `SyncEntity` (app enum, `sync/event.rs:38`, derives `JsonSchema` for the
  codegen contract — must stay app-side) and `SyncAction` (`sync/event.rs:232`,
  app-side) are named in the trait the task wants moved into `ziee-auth`.
- 24 call sites pass `SyncEntity::{User,Group,Profile,Session,SessionSettings,
  AuthProvider}` + `SyncAction::{Create,Update,Delete}`.
- **Resolution (in-move transform):** replace with a ziee-auth-local
  `AuthSyncEntity` + `AuthSyncAction` enum (or `&str`) in the trait; the app-side
  `PublishSyncSink` maps `AuthSyncEntity::User → SyncEntity::User` etc. before the
  real `publish`. Equivalence-preserving (same events, same audiences); this is a
  declared TRANSFORM, doable inside BA.

---

## Additional wrinkle for the auth-only build DB (not a blocker, but scoping)

The auth `query!` macros touch columns added by ALTER migrations, not just the
create-table ones — e.g. `repository.rs:58` selects `users.password_changed_at`
(added by mig **64**). So the auth-only migration set = *every migration that
creates OR alters an auth table/column an auth query references* (mig 1, 27, 44,
46, 47, 48, 64, 125, 129, 130, and any ALTER-users the queries hit), NOT merely
the six create-table files. Each auth `query!` needs a per-query audit to fix the
exact set so the auth-only build DB verifies with zero "column does not exist"
errors. This is bounded work, but it must be done before the auth-only build DB
lane is green.

---

## Correct next step (a "BG-round-2" then BA-full — same shape as BA→BG)

1. **BG-2a (SDK, mechanical):** move `utils/url_validator.rs` → `ziee-framework`
   (build-DB-free) + shim `crate::utils::url_validator`; `ziee-auth`/oauth2/apple
   name `ziee_framework::...`. E8 golden must stay byte-identical.
2. **BG-2b (DECISION + SDK):** decide `common::secret`'s SDK home (recommend
   `ziee-framework`, sibling to url_validator — both domain-free, build-DB-free),
   move + shim `crate::common::secret`.  ← needs human ratification (unplanned).
3. **BA-full then runs unblocked:** move `modules/auth/*` + user
   repo/models/permissions/events/types into `ziee-auth`; apply the C3 scope
   choice (keep user admin handlers app-side, or add `AuthUserCleanup`); apply
   the C4 `AuthSyncEntity` transform; stand up the auth-only build DB (per-query
   migration-set audit); relocate the auth migration files (checksum-preserved);
   wire build-time merged-migrator composition in `build.rs` + `core/database/mod.rs`;
   verify `pg_dump --schema-only` == `.extraction/baseline/schema.sql`; golden both
   surfaces.

The golden-spike (BA/) + this map + the migration-composition design
(BA/TRANSFORMS.md §N3) de-risk step 3 substantially — the remaining unknowns are
the C1 home decision and the per-query auth-migration-set audit.

## Why not force it now
Doing steps 1–2 *inside* BA-full would (a) relocate app-wide infra (11 + 18
consumers) into the SDK on my own architecture call for `common::secret` (the
plan never scoped it), and (b) leave a large, schema-bound, half-green tree that
can't be committed if any of the many build-loop iterations (auth-only build DB
provisioning + ~40 `query!` verifications + merged-migrator + pg_dump) fails
mid-way — the precise "broken change" the task forbids. Landing BG-2 as its own
golden-verified in-ziee/SDK chunk first is the safe, reversible path.
