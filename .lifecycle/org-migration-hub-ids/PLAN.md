# PLAN — org-migration-hub-ids

ORG-MIGRATION PHASE 2: rebrand the hub **publisher namespace** from
`io.github.phibya/*` → `io.github.ziee-ai/*` across (a) the vendored hub seed,
(b) an idempotent DB migration for already-installed entities, (c) the hub tests
+ `hub_manager` refs, and (d) the upstream `ziee-ai/hub` source YAML (two-PR
workflow). Exactly **12 entities** carry this namespace: 5 assistants
(code-reviewer, creative-writer, deep-researcher, sql-helper, vision-analyst) +
7 models (llama-3-1-8b-instruct, llama-3-2-3b-instruct-gguf,
qwen2.5-coder-7b-instruct, qwen2.5-vl-3b-instruct, phi-3-mini-4k-instruct,
nomic-embed-text-v1-5-gguf, deepseek-r1-70b).

## Items

- **ITEM-1**: Rename the seed publisher directories `git mv src-app/server/resources/hub-seed/assistants/io.github.phibya → io.github.ziee-ai` and `.../models/io.github.phibya → io.github.ziee-ai`. (The `binaries/hub-seed/` mirror is gitignored + regenerated from `resources/` by `build_helper/hub_seed.rs` at build time — do NOT touch it.)
- **ITEM-2**: Rewrite `src-app/server/resources/hub-seed/index.json` — the 12 `name` fields and 12 `manifest_path` fields whose namespace is `io.github.phibya` → `io.github.ziee-ai`. Keep `hub_version: "2.0.0"` unchanged.
- **ITEM-3**: Rewrite the 12 relocated manifest JSONs — each entity's own `name` field plus every `dependencies[].name` cross-reference to a `io.github.phibya/*` model (21 total string occurrences) → `io.github.ziee-ai/*`.
- **ITEM-4**: Add migration `src-app/server/migrations/00000000000131_rewrite_hub_ids_phibya_to_ziee_ai.sql` — idempotent `UPDATE hub_entities SET hub_id = replace(...)` for `hub_id LIKE 'io.github.phibya/%'`, mirroring migration 92's guarded/idempotent shape. `hub_entities.hub_id` is the sole table storing the raw catalog id string.
- **ITEM-5**: Update the `#[cfg(test)]` assertions in `src-app/server/src/modules/hub/hub_manager.rs` (6 refs: `is_safe_name` accept case + the `HUB_SEED.get_file(...)` seed-path/name assertions) to the renamed `io.github.ziee-ai` paths and names.
- **ITEM-6**: Update hub integration tests to the rebranded ids: `tests/hub/catalog_v1.rs` (6 refs), `tests/hub/mod.rs` (5 refs), `tests/llm_model/download_test.rs` (1 ref — `INCOMPATIBLE_FIXTURE_NAMES`).
- **ITEM-7**: Update `tests/hub/migration_test.rs` to the post-all-migrations final state (`io.github.ziee-ai/*` after 92→131 both run) AND add an assertion that migration 131 rewrites a seeded `io.github.phibya/*` `hub_entities.hub_id` to `io.github.ziee-ai/*` and is idempotent.
- **ITEM-8**: Update the self-contained slug-generation unit-test fixtures in `src-app/server/src/modules/workflow_mcp/tools.rs` (input `io.github.phibya/research-summarize-write` → `io.github.ziee-ai/...` and the expected `wf_io_github_*` slug), for rebrand consistency.
- **ITEM-10**: Unblock the pre-existing catalog_v1 failures in the touched test file so the rebrand is verifiable end-to-end. Five `catalog_v1` tests (version / index / 3× manifest) fail with 403 on the **base branch too** (proven — see DRIFT-2) because they grant `hub::models::read` while the `/hub/{version,index,manifest}` handlers require `HubCatalogRead`; fix those 5 grants to `hub::catalog::read`. Also bump the stale seed-count asserts (`SEED_ITEM_COUNT` 28→29, `workflows` 9→10) to the actual seed content (10 workflows). Do NOT touch the `refresh`/`installed` tests (they legitimately use `hub::models::read`).
- **ITEM-9**: In a clone of `ziee-ai/hub` (`/data/pbya/ziee/tmp/hub-clone`), on branch `chore/org-migration-publisher-ids`, edit the 12 source YAMLs (`assistants/*.yaml`, `models/*.yaml`): `name: io.github.phibya/*` → `io.github.ziee-ai/*` plus each `dependencies[].name` cross-ref. Leave `_hub_curation.contributor: phibya` (provenance, decoupled) and README/CONTRIBUTING (external URL / illustrative example). Push + open PR (merges together with the ziee PR).

## Files to touch

- `src-app/server/resources/hub-seed/assistants/io.github.phibya/**` → renamed dir (5 manifests)
- `src-app/server/resources/hub-seed/models/io.github.phibya/**` → renamed dir (7 manifests)
- `src-app/server/resources/hub-seed/index.json`
- `src-app/server/migrations/00000000000131_rewrite_hub_ids_phibya_to_ziee_ai.sql` (new)
- `src-app/server/src/modules/hub/hub_manager.rs`
- `src-app/server/src/modules/workflow_mcp/tools.rs`
- `src-app/server/tests/hub/catalog_v1.rs`
- `src-app/server/tests/hub/mod.rs`
- `src-app/server/tests/hub/migration_test.rs`
- `src-app/server/tests/llm_model/download_test.rs`
- (separate repo) `hub-clone/assistants/*.yaml`, `hub-clone/models/*.yaml`

## Patterns to follow

- **Migration** — mirror `migrations/00000000000092_rewrite_hub_entities_hub_id_to_reverse_dns.sql` exactly: header comment explaining intent, single guarded `UPDATE hub_entities` with an idempotency `WHERE` clause, optional `RAISE NOTICE` diagnostic. This is the direct precedent (a reverse-DNS `hub_id` rewrite touching only `hub_entities`).
- **Seed shape** — the renamed dirs + JSON must match the existing seed layout (`<category>/<namespace>/<leaf>/<version>.json`) consumed by `hub_manager.rs::HUB_SEED` (`include_dir!`) and validated by `seed_index_version_matches_const`.
- **Hub-repo source** — mirror the existing flat-YAML shape (`_hub_curation` + envelope with `name:` reverse-DNS + `dependencies[].name`); the Pages build (`scripts/build-pages.py::split_name`) derives the namespace from `name`, and `scripts/validate.py` / `scripts/test-pages-build.sh` are the verification gate. Two-PR hub workflow per [[feedback_hub_repo_clone_first]].
- **Test edits** — pure string-swap of the rebranded id in the existing assertions; no new test *scaffolding* except the migration-131 coverage added to `migration_test.rs` (mirror its existing insert→run→assert helper).
