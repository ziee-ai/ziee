# DECISIONS — org-migration-hub-ids

### DEC-1: Which entities carry the `io.github.phibya` publisher namespace to rebrand?
**Resolution:** exactly 12 — 5 assistants (code-reviewer, creative-writer, deep-researcher, sql-helper, vision-analyst) + 7 models (llama-3-1-8b-instruct, llama-3-2-3b-instruct-gguf, qwen2.5-coder-7b-instruct, qwen2.5-vl-3b-instruct, phi-3-mini-4k-instruct, nomic-embed-text-v1-5-gguf, deepseek-r1-70b). The 6 mcp-servers keep their upstream namespaces (io.github.github, com.brave, io.github.modelcontextprotocol, app.linear).
**Basis:** codebase — grep of `resources/hub-seed/{assistants,models}/io.github.phibya` + index.json `name` fields; matches the task decision (5 assistants + 7 models).

### DEC-2: Rebrand `_hub_curation.contributor: phibya` too?
**Resolution:** No — leave `contributor: phibya` unchanged.
**Basis:** codebase — `contributor` is curation provenance (who submitted the entry), decoupled from the published `name` namespace: the 6 mcp-servers carry `contributor: phibya` with non-`phibya` names. It never reaches the seed (the built index.json/manifests carry `author`, not `contributor`), so the consumer/migration is unaffected. Out of scope for a publisher-ID (namespace) migration.

### DEC-3: Bump `hub_version` / `SEED_HUB_VERSION` from 2.0.0?
**Resolution:** No — keep `2.0.0` in both `resources/hub-seed/index.json` and the `SEED_HUB_VERSION` const.
**Basis:** codebase — `build-pages.py` documents `hub_version` as "bumped on **schema** changes, NOT per entry". A publisher rename is content, not schema. Keeping it constant preserves the `seed_index_version_matches_const` lockstep invariant (on-disk index `hub_version` must equal the const).

### DEC-4: Migration number?
**Resolution:** `00000000000131`.
**Basis:** codebase — `ls migrations/ | sort | tail -1` = `00000000000130_*`; 131 is the next free slot.

### DEC-5: Which tables does the migration rewrite?
**Resolution:** Only `hub_entities.hub_id`.
**Basis:** codebase — it is the sole column storing the raw reverse-DNS catalog id (migrations 8/69/79/80/92 are the only ones touching it; `repository.rs`/`handlers.rs` insert the `name` directly). Installed entities reference the catalog only via `hub_entities.entity_id` (UUID). Mirrors migration 92, which likewise touched only `hub_entities`.

### DEC-6: Migration idempotency + shape?
**Resolution:** `UPDATE hub_entities SET hub_id = 'io.github.ziee-ai/' || substring(hub_id from length('io.github.phibya/')+1) WHERE hub_id LIKE 'io.github.phibya/%';` (prefix-scoped; a second run matches nothing). Include a header comment + a `RAISE NOTICE` reporting the rewritten count, matching migration 92's style.
**Basis:** convention — migration 92's guarded/idempotent `WHERE` pattern.

### DEC-7: Update the `workflow_mcp/tools.rs` slug-test fixture (`io.github.phibya/research-summarize-write`)?
**Resolution:** Yes — swap the fictional example input to `io.github.ziee-ai/research-summarize-write` and its expected `wf_io_github_ziee-ai_research-summarize-write` slug.
**Basis:** convention — full-rebrand consistency (no `io.github.phibya` left in tree); it is a self-contained slug-transform unit test with a mechanically-derivable expected value, and `slug_for_name` preserves hyphens.

### DEC-8: Rebrand the gallery `crawl.json` recorded fixtures (ui + desktop, 45 refs each)?
**Resolution:** Defer — NOT in this branch; flag as a follow-up for the frontend/gallery workstream.
**Basis:** convention — the crawl cassettes are a frontend-gallery concern with their own visual-baseline gate; no asserting test depends on the `io.github.phibya` string (repo-wide grep confirms), so there is zero correctness impact, and mixing a fixture reword into the seed/migration branch would risk unrelated visual-snapshot churn. Keeps this change focused + cleanly testable.

### DEC-9: Hub-repo (`ziee-ai/hub`) workflow?
**Resolution:** Clone via `gh` (HTTPS/token — SSH has no key here), branch `chore/org-migration-publisher-ids`, edit the 12 source YAMLs' `name:` + `dependencies[].name` to `io.github.ziee-ai/*`, leave `contributor`/README/CONTRIBUTING, run `validate.py` + `test-pages-build.sh`, push + `gh pr create`. The two PRs (hub + ziee) merge together.
**Basis:** user/[[feedback_hub_repo_clone_first]] — the established two-PR hub workflow; SSH clone returned "Permission denied (publickey)" while `gh` is authenticated.

### DEC-11: Fix the pre-existing catalog_v1 failures the rebrand surfaced?
**Resolution:** Yes — minimal + correct: change the 5 stale `hub::models::read` grants (version/index/3× manifest tests) to `hub::catalog::read`, and bump `SEED_ITEM_COUNT` 28→29 + `workflows` 9→10. Leave the `refresh`/`installed` tests' `hub::models::read` untouched.
**Basis:** codebase + evidence — the same 5 tests fail identically (403) on the rebrand-free base checkout (proven in DRIFT-2), so they are pre-existing (a hub-permission refactor moved the catalog reads behind `HubCatalogRead` without updating these grants; the sibling `catalog_read_cannot_refresh` test confirms `hub::catalog::read` is the correct read perm). The count bump reflects the actual seed (29 items / 10 workflows). Fixing them (rather than skipping) unblocks end-to-end rebrand verification and honors "no red in a touched file" ([[feedback_no_ignore_unless_platform]]).

### DEC-10: Run `cargo clean` after adding the migration?
**Resolution:** No — a fresh `cargo check -p ziee` suffices in this new worktree; only fall back to `cargo clean` if sqlx emits a "relation does not exist" error.
**Basis:** codebase — the per-worktree build DB (`ziee_build_<key>`) is provisioned + migrated from scratch on the worktree's first build (it inherits no stale schema), and the migration adds no schema/`sqlx::query!` surface, so a rebuild picks it up without a clean.
