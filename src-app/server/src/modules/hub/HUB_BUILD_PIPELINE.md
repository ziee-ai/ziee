# Hub `ziee-ai/hub` build pipeline (spec)

This document specifies the build pipeline that lives in the **separate**
`ziee-ai/hub` repository. It is not implementable in this repo. It exists
here for cross-team coordination: when the upstream pipeline ships, the
ziee consumer in `hub_manager.rs` and the embedded seed under
`resources/hub-seed/` will be the consumers of its output.

## Schema versioning convention

Schemas live under a **date-stamped directory** (`schemas/<YYYY-MM-DD>/`),
mirroring the upstream MCP convention
(`static.modelcontextprotocol.io/schemas/2025-09-29/...`). A new dated
directory is published whenever a breaking schema change ships; the
catalog's `schema_version: u32` then bumps in lockstep. Throughout this
doc, `schemas/2026-06-12/` is the current dated directory — replace with
the live date when the pipeline rebuilds.

## Goals

1. Publish a **static, per-entry-versioned registry** as a GitHub Pages
   site. The layout is the same one ziee's `hub_manager.rs` consumes
   (see `Pages layout` below).
2. **Ingest the official MCP registry**
   (`https://registry.modelcontextprotocol.io/v0/servers`), filter to
   entries ziee can actually run, and merge them into the published
   catalog alongside ziee-native entries.
3. **No cosign / signed-tarball** path. Trust = HTTPS to GitHub Pages.
4. Provide a build snapshot that ziee's `scripts/sync-hub-seed.sh`
   (future) pulls from to regenerate `resources/hub-seed/`.

## Pages layout (what the build branch must serve)

```
/index.json                                # the Catalog
/schemas/2026-06-12/*.schema.json          # versioned JSON Schemas
/models/<id>/<version>.json                # full per-entry manifest
/assistants/<id>/<version>.json
/mcp-servers/<id>/<version>.json           # a server.json + _meta envelope
```

The catalog (`index.json`) carries lightweight `IndexItem` envelopes
(`id` + `name` + `version` + `qualified_name` + `_meta` + tags +
summary). The full manifest (with model files / instructions /
packages / remotes) lives at `<type>/<id>/<version>.json` and is
fetched **lazily** only when the consumer opens or installs an entry.

## Source on `main`

Per-entry manifests authored in the catalog envelope:

```
ziee-ai/hub/
├── models/<id>.yaml                       # one YAML per ziee-native model
├── assistants/<id>.yaml
├── mcp-servers/<id>.yaml                  # a server.json (YAML-formatted) + _meta
├── schemas/2026-06-12/                    # JSON Schemas (date-stamped)
└── scripts/build-pages.{ts,rs}            # the publisher script
```

YAML is the maintainer-facing format (comments, trailing commas);
the build emits JSON.

## Build script

A Rust binary OR a Node script (TBD by the hub repo's maintainers). The
choices are equivalent — both have well-supported JSON Schema validators
and HTTP clients. Recommend Rust for type-shared structs with ziee.

The script performs five steps in order:

1. **Load + validate source manifests.** Read every `*.yaml` under
   `models/`, `assistants/`, `mcp-servers/`. Validate each against the
   matching `schemas/2026-06-12/*.schema.json`. Reject + fail the build
   on any schema violation — the goal is "if it's in `main` it's
   installable."

2. **Ingest the official MCP registry.** Paginate
   `GET https://registry.modelcontextprotocol.io/v0/servers`. Use the
   `updated_since` query param for incremental runs (the previous run's
   max `updatedAt` stored as a build cache). For each ingested entry:

   - **Filter to installable.** Keep ONLY:
     - `packages[]` entries where `registryType ∈ {npm, pypi}` AND
       `runtimeHint ∈ {npx, uvx}` AND transport is `stdio` (or
       implicitly stdio = the default).
     - `remotes[]` entries where `type ∈ {streamableHttp, sse}`.
   - **Drop** anything in `docker`, `oci`, `mcpb`, `dnx`. Log dropped
     count to the build log (visible in CI) so contributors can see why
     a server they expected didn't show up.
   - Add `_meta["ai.ziee.hub"] = { "source": "mcp-registry",
     "ingested_at": "<iso8601>" }` and preserve any existing
     `_meta["io.modelcontextprotocol.registry"]` keys so the frontend
     can render a provenance badge.
   - Keep the official reverse-DNS `name` as `qualified_name`; derive
     ziee's slug `id` from a normalized lowercase form (e.g.
     `io.github.modelcontextprotocol/server-filesystem` → `filesystem`).
     If two ingested entries collide on slug, prefer the one with the
     higher `popularity_score` and log the drop.

3. **Merge ziee-native + ingested.** Union the two sets, ziee-native
   wins on slug collision. Sort by category + slug for deterministic
   output.

4. **Emit the Pages layout** to a build output dir:
   - `index.json` containing `Catalog { schema_version, hub_version,
     generated_at, items[IndexItem] }`.
   - `<type>/<id>/<version>.json` for each entry, full body + envelope.
   - `schemas/2026-06-12/*.schema.json` copied verbatim from `main`.

5. **Commit to `gh-pages`.** Force-overwrite the branch with the build
   output (one commit per build, no history pressure — Pages branches
   are conventional that way). The latest build is what `<base>/...`
   serves.

## GitHub Action

Trigger on:

- `push` to `main` — publishes contributor-authored changes immediately.
- `schedule` daily cron — re-pulls the MCP registry so newly-published
  servers appear without a manual push.
- `workflow_dispatch` — manual rerun (e.g. after a registry hotfix).

No cosign / sigstore step. No artifact upload. Just `git push -f` to
`gh-pages`.

## Pages settings (one-time)

In `ziee-ai/hub`'s GitHub settings, enable Pages from the `gh-pages`
branch. The default URL `https://ziee-ai.github.io/hub/` is what
`hub_manager.rs`'s `DEFAULT_PAGES_BASE` resolves to.

## ziee seed sync

A future `scripts/sync-hub-seed.sh` in this repo will:

1. Pull the latest published `index.json` + per-entry files from
   `https://ziee-ai.github.io/hub/`.
2. Stage them under `resources/hub-seed/`.
3. Show a diff for the maintainer to review before committing (the seed
   bumps should be a deliberate version bump, not silent drift).

Until that script exists, maintainers regenerate the seed manually by
mirroring the Pages layout to `resources/hub-seed/`.

## Out of scope (this pipeline)

- A2A Agent Card ingestion (different ecosystem, different envelope).
- Third-party / multi-repo federation (drat-style overlays). The
  registry is single-source.
- Skills / workflows bundles with code + assets. These will need a
  size-aware download path (probably per-entry tarballs), which can
  land as a later amendment.
- Per-entry cosign signatures. Trust is HTTPS to Pages; the hub repo's
  branch protection rules are the meaningful trust boundary upstream.

## Versioning rules

- The catalog's `hub_version` is a build marker — bumped on schema
  changes (date-stamped directory rolls forward + `schema_version`
  increments), NOT on every entry update.
- Each entry's `version` is independent semver. Bumping a model from
  `1.0.0` → `1.0.1` flags it as "update available" for users with an
  installed `1.0.0`. The consumer never auto-updates — admins click
  through.
- The seed in ziee carries a snapshot's `hub_version`; the runtime
  test `seed_index_version_matches_const` cross-checks that against
  the embedded constant so a manual seed edit can't drift silently.

## Testing automation

Two `just` recipes cover the publisher + consumer ends of the pipeline:

| Command (cwd)                           | What it does |
|-----------------------------------------|--------------|
| `just test-pages` (ziee-ai-hub)         | Runs the GitHub Pages workflow locally via `act` + Docker, asserts the produced `dist/` tree (item count, reverse-DNS names, MCP entries pass strict server.json shape). **Hard-fails** if Docker is missing or not running. |
| `just test-hub` (ziee)             | Runs the full hub-related integration suite (`hub::`, `assistant::`, `mcp::`, `llm_model::` filters) against the isolated `hubreg_build` Postgres DB. Saves a timestamped log per CLAUDE.md memory. **Hard-fails** if `tests/.env.test` is missing. |
| `just check-hub` (ziee)            | Compile gate: `cargo check -p ziee --all-targets`. Fast. |
| `just tsc` (ziee)                  | Compile gate: `npx tsc --noEmit` on both `src-app/ui` and `src-app/desktop/ui`. |

Both repositories are self-hosting their own automation. No user input is
required to run them; CI can shell out to the recipes directly.

## Upgrade path for pre-reverse-DNS hub installs

Existing `hub_entities` rows from before the reverse-DNS migration
carried a slug `hub_id` (e.g. `"filesystem-mcp"`). The SQL migration
`00000000000092_rewrite_hub_entities_hub_id_to_reverse_dns.sql` runs at
boot via build.rs and rewrites every recognized legacy slug to its
reverse-DNS form (e.g.
`"io.github.modelcontextprotocol/filesystem"`). The migration is
**idempotent** (rows whose `hub_id` already contains `/` are skipped)
and emits a `RAISE NOTICE` for any orphan slug it can't map — those rows
remain installable via the regular settings pages, but the user must
reinstall to re-track them in the Updates view.
