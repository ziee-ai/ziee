# BASE — upstream-port

Conflict surface against CURRENT `upstream/main` (`b1147a242`), measured in this
worktree before any code was written.

## Branch base

`upstream/khoi` was `d65308170` and is an ANCESTOR of `upstream/main`
(`git branch -r --contains d65308170` lists both), so bringing it up to date was a
pure fast-forward — 43 files, zero conflicts. The brief's "on any conflict while
updating, prioritise upstream/main" rule therefore never had to fire.

## Migration numbers

**This branch adds NO migration.** Recorded anyway so a later reader can see the
check was made:

```
find src-app/server -path '*/migrations/*.sql' -printf '%f\n' | cut -d_ -f1 | sort -n | tail -2
  202608210100
  202608250100          <- current upstream server max
```

## OpenAPI regen

**Not implied.** No `JsonSchema` request/response type and no route SIGNATURE
changes. ITEM-3 changes only *whether* a router is merged, not the shape of any
route it declares, so the emitted spec for an enabled deployment is unchanged. A
non-empty diff in `src-app/ui/openapi/openapi.json` or either
`api-client/types.ts` would mean an out-of-scope hunk was dragged in, and is treated
as a defect rather than as a regen to run.

Note `just` is not installed on this box, so if a regen were ever needed it would
have to be the raw two-command form from `justfile:550-554`.

## Submodules

`sdk` `4ab75300` (branch `chat`), `agent-kit` `e07b25308`, pgvector `cab9da72`.
**This branch moves none of them**, which is what keeps the GPU/CUDA fix and the CORS
`create_cors_layer_with` union out of scope — both live only on the sdk `paws` line
and are escalated instead (see PLAN.md `## Out of scope`).

`ITEM-2` does depend on a symbol crossing the crate boundary
(`ziee::CHAT_STREAM_CONNECTION_HEADER`), but that is a `pub` + re-export INSIDE this
repo, not an sdk change.

## Files the other in-flight work touches

The peer worker on this box is on paws' `fix/paws-ui-polish`, in a different
repository line entirely, so there is no overlap. Within `ziee-ai/ziee`, `khoi` is
the only branch this session touches and nothing else is being pushed to it.

## Pre-existing RED tests on `upstream/main` — measured, not assumed

While verifying the port I ran two unit tests in this worktree (which is
`upstream/main` plus this branch's changes, none of which touch either file) and both
FAIL:

```
modules::llm_repository::utils::tests::capability_url_targets_the_kinds_listing_surface
  left:  Some("http://127.0.0.1:1520/models/api/models?limit=1")
  right: Some("http://127.0.0.1:1520/api/models?limit=1")
modules::background_mcp::tools::argument_contract_tests::every_spawn_refusal_is_actionable
```

So `ziee-ai/ziee` `main` is red on these today. Upstream has no PR CI (only two
tag-triggered release workflows), which is how they survived. Both are repaired by
ITEM-11, added to this branch after the audit found them.
