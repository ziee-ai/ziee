# PLAN_AUDIT — remote-model-picker

Audited against the code in the worktree (read-only investigation).

## Breakage risk

- ITEM-1/2 change the add-drawer internals only; submit still flows through `createLlmModel`→`ApiClient.LlmModel.create` (unchanged contract). No caller breaks.
- ITEM-5 (new provider_type) is additive to a CHECK constraint + allowlist + a match arm that already has a `custom` catch-all → no existing provider row invalidated.
- ITEM-7 only adds fields onto `DiscoveredModel` values already returned; id-only consumers keep working.
- ITEM-8/9 set a column that already exists and is currently always-false; readers already deserialize `is_deprecated`. The DEC-5 guard prevents mass-false-flagging on failed/empty fetch. Verified: `create_model` (`handlers/models.rs:145`) receives only `provider_id`, not `provider_type` → ITEM-9 must add a provider read.

## Pattern conformance

- Sweep folds into the existing `llm_model/prune.rs` (verified: 6h tick, best-effort, boot reconcile, spawned at `llm_model/mod.rs:68`). Discovery reuses `fetch_v1_models`; provider-type follows the established 3-site threading. UI reuses kit `Combobox`/badges. Sync emit verified: models are permission-scoped — `create_model` emits `sync_publish(SyncEntity::LlmModel, …, Audience::perm::<LlmModelsRead>)` + `sync_publish(SyncEntity::UserLlmProvider, …, Audience::perm::<UserLlmProvidersRead>)`; the sweep replicates that pair with `origin=None`.

## Migration collisions

- Highest existing migration is `00000000000131`; the plan uses `00000000000132` — no collision. Single migration: replace the additive CHECK constraint + one seed INSERT. Requires `cargo clean` so build.rs re-applies migrations for sqlx verification (per CLAUDE.md).

## OpenAPI regen

- `DiscoveredModel`/`DiscoverModelsResponse` are unchanged (ITEM-7 fills existing fields) ⇒ discovery types need no regen. The new reconcile endpoint (ITEM-8/ITEM-10) is a new route → requires `just openapi-regen` for BOTH ui and desktop, plus `npm run check` in both workspaces.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — self-contained drawer change; Combobox not creatable, handled via a custom-id toggle fallback.
- **ITEM-2** — verdict: PASS — replaces the hardcoded capabilities object with mapped values; create contract unchanged.
- **ITEM-3** — verdict: PASS — new store action over an existing generated client method.
- **ITEM-4** — verdict: CONCERN — depends on ITEM-8's reconcile endpoint existing; sequence ITEM-8 before wiring ITEM-4.
- **ITEM-5** — verdict: CONCERN — string enum needs no openapi regen, but the new migration must be applied to the build DB (`cargo clean`) for sqlx verification.
- **ITEM-6** — verdict: PASS — additive list/icon entry.
- **ITEM-7** — verdict: CONCERN — additive parse; keep SSRF guards. OpenRouter's `/api/v1/models` shape is asserted from public docs, not yet verified against a live response — Phase 5 captures a fixture and confirms field paths before relying on them.
- **ITEM-8** — verdict: CONCERN — new reconcile route ⇒ openapi-regen both binaries; must gate on a successful non-empty fetch (DEC-5). Desktop concern resolved: `llm_model/prune.rs` already runs unconditionally on desktop via `mod.rs::init()`.
- **ITEM-9** — verdict: CONCERN — catalog lookup is available, but `provider_type` is not in `create_model` scope; requires an added `Repos.llm_provider.get_by_id` read + a post-create `set_model_deprecated` call.
- **ITEM-10** — verdict: PASS — verified spawn site `llm_model/mod.rs:68`; folding the job into `prune.rs` means the existing spawn already drives it; only the reconcile route is genuinely new.
