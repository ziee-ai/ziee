# DRIFT-2 — merge-prep reconciliation (origin/main 07a3e9477)

Merged current `origin/main` (181 commits ahead) into the branch for the
merge-gate. Reconciling the plan against the merged base.

- **DRIFT-2.1** — verdict: resolved — Migration renumber 133/134 → **151/152**: main advanced its max migration to 150 (`..148_add_file_rag_reranker`, `..149_create_file_index_state`, `..150_add_retrieval_limits`), so the branch's `00000000000133_create_voice.sql` / `00000000000134_grant_voice_permissions_to_users.sql` collided. `git mv`'d to `00000000000151_create_voice.sql` / `00000000000152_grant_voice_permissions_to_users.sql` (create-before-grant order preserved); updated every voice code/test comment + PLAN.md/DECISIONS.md reference (133→151, 134→152). `knowledge_base`/`js_tool` "migration 134" comments are main's own migrations — left untouched. build.rs applies all in order; `Repos.voice` sqlx queries verify against the migrated build DB.
- **DRIFT-2.2** — verdict: resolved — `core/repository.rs` registration-list conflict: main added `js_tool: JsToolRepository` at the same slot the branch added `voice: VoiceRepository`. Kept BOTH (js_tool then voice). `modules/mod.rs` auto-merged both `pub mod` lines cleanly.
- **DRIFT-2.3** — verdict: resolved — Generated files (`ui/` + `desktop/ui/` `api-client/types.ts`, `testIds.generated.ts`, gallery `galleryCoverage`/`stateMatrix`/`overlay-registry`/`STATE_MATRIX.md`) conflicted; resolved by taking main's version then REGENERATING (`--generate-openapi` both binaries + the npm generators), never hand-editing — so they deterministically re-include the voice types/testids/gallery cells on top of main's.
- **DRIFT-2.4** — verdict: resolved — Shared-target contamination: `src-app/target` symlinks to `lazyload-target`, where a now-removed `integ-wt` worktree cached its build.rs OUT_DIR, so the warm build resolved the hub-seed to a dead `integ-wt` path. Isolated this worktree onto a dedicated `CARGO_TARGET_DIR` for a genuine clean build (also the phase-8/merge-gate clean-build requirement).

**Unresolved drifts:** 0
