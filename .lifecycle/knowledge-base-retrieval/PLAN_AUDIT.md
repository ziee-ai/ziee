# PLAN_AUDIT — knowledge-base-retrieval

Audited against the codebase (base @ 4a3769691, highest migration 132) AND against
the three-angle adversarial audit that drove this revision. Every prior CRITICAL/
HIGH finding is now addressed by an item, decision, or test (cross-referenced).

## Prior-audit findings → resolution

- Highlight on-demand-search fatal flaw → **Part C rewritten to ingest-time geometry** (ITEM-22/23/24/25, DEC-31).
- Index-status has no backend state/emit → **Part I** (ITEM-11/12/13, DEC-26 revised).
- `document_count` drift on external delete → **derived at read** (ITEM-14/18, DEC-32); UI refetch on `sync:file` (ITEM-27).
- chat-extension order 24 collides with `summarization` → **order 23** (ITEM-21).
- cross-user `search_knowledge` leak → owner-filtered `resolve_scope_file_ids` (ITEM-18/20) + TEST.
- attach-existing never indexed → reindex-on-attach when 0 chunks (ITEM-18) + ITEM-13.
- scanned/zero-text docs → `no_text` terminal state (ITEM-11/12/17/31, DEC-35).
- duplicate re-drops → checksum dedup + report (ITEM-18/19/31, DEC-36).
- half-indexed silent answers → `indexing_incomplete` signal in tool + transparency panel (ITEM-20/36, DEC-37).
- per-file size cap + batch-reject UX → server-side validation + itemized report (ITEM-19/31, DEC-33).
- reranker off/undiscoverable → **hub-delivered model** (Part H) + admin nudge (ITEM-38, DEC-38).
- search result cap / truncated semantics / pagination / text-rects ownership / regen ordering → ITEM-20/7/18/25/39.

## Breakage risk

- Reranker additive + gated OFF; trait default keeps callers compiling; proxy allowlist additive; retrieval rerank gated so `files_mcp` byte-identical until opt-in.
- **Part I touches the shared `file_rag` ingest** (write status + emit) — additive (new table + writes at existing transition points); no behavior change to chunking/retrieval; the no-text early-return becomes an explicit status write (same control flow).
- **Part C touches the shared PDF extract path** (`clean_extracted_text` now also emits a geometry array) — the cleaned-string output is unchanged (existing text consumers unaffected); geometry is a new side-output. Migration + storage are additive; backfill is idempotent.
- **Hub schema change** (`additionalProperties:false` + `rerank`) is backward-compatible (new optional bool); the seed mirror must stay in lockstep with `SEED_HUB_VERSION` (build panics on drift — a known contract).
- KB reuse of `semantic_search` is call-only; `file_chunks` untouched by K; deleting a KB/doc never touches shared chunks (no `kb_id` on `file_chunks` — verified).
- Chat integration is registry-based; the one core edit (`PanelRendererMap['file']` +optional `{page,charRange}`) is additive-optional.
- Order 23 verified free (20 file, 22 control_mcp, 23 FREE, 24 summarization, 25 memory…30 mcp).

## Pattern conformance

- Reranker mirrors embedding capability; hub model mirrors `nomic-embed…yaml`; index-state mirrors file_rag ingest + owner-scoped `publish`; geometry mirrors text-page storage; KB + UI mirror the named reference surfaces. All conform.

## Migration collisions

- Highest = 132. New: 133 (KB tables), 134 (grant), 135 (reranker settings), 136 (file_index_state), 137 (page geometry). Sequential, no collision, all additive. `cargo clean` after adding (build-DB note).

## OpenAPI regen

- New types across R/I/K/C + `SyncEntity` variants ⇒ `just openapi-regen` BOTH workspaces (ITEM-39), **sequenced before frontend** (execution-order constraint stated). Golden `emit_ts` enforces consistency. `/mcp` + `/rerank` proxy routes excluded (plain routes).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — DTO+trait-default+OpenAI `/v1/rerank`; path reconciled (DEC-30).
- **ITEM-2** — verdict: PASS — JSONB field + allowlist + guard + hub-map at `handlers.rs:1612`.
- **ITEM-3** — verdict: PASS — mirrors `embed` + auto_start.
- **ITEM-4** — verdict: CONCERN — argv/inject/proxy triad; verify `--reranking`+`--pooling rank` at impl (DEC-30).
- **ITEM-5** — verdict: PASS — ADD COLUMN mirrors migration 99.
- **ITEM-6** — verdict: CONCERN — compile-checked positional `query_as!` (SET+RETURNING per column); compiler+TEST guard; mutual-exclusion intent tested.
- **ITEM-7** — verdict: CONCERN — behavioral; candidate pool must be **wider than top_k** and `truncated` recomputed; gated OFF; fallback-safe. TEST proves promotion-from-outside-top_k.
- **ITEM-8** — verdict: PASS — additive optional schema field, both versions.
- **ITEM-9** — verdict: PASS — mirrors the existing embedding manifest.
- **ITEM-10** — verdict: CONCERN — hub build-pipeline regen + seed mirror + `SEED_HUB_VERSION` lockstep (build panics on drift); two-PR coordination.
- **ITEM-11** — verdict: PASS — additive table; the missing state the audit demanded.
- **ITEM-12** — verdict: CONCERN — writes at ingest transitions in the SHARED path; must not change chunk/retrieval behavior; emit owner-scoped. TEST drives the real worker to completion.
- **ITEM-13** — verdict: PASS — reuses `spawn_reindex`.
- **ITEM-14** — verdict: PASS — additive tables; no denormalized count (derive at read).
- **ITEM-15** — verdict: PASS — idempotent grant mirrors 104.
- **ITEM-16** — verdict: PASS — module entry + loopback upsert mirror `web_search`.
- **ITEM-17** — verdict: PASS — status from `file_index_state` (now real).
- **ITEM-18** — verdict: CONCERN — the correctness core: owner-filtered scope (cross-user guard), dedup, reindex-on-attach, remove-join-only, paginated list, count-via-subquery. Each pinned by a TEST.
- **ITEM-19** — verdict: PASS — cap/422 + server-side size/type + dedup report mirror `attach_file_capped`/project upload; `get_by_id_and_user` owner-scope.
- **ITEM-20** — verdict: PASS — reranked `semantic_search`; owner-filtered; 1 MB cap; indexing-incomplete signal.
- **ITEM-21** — verdict: PASS — order 23 free; two `mcp.rs` edits (TEST-guarded).
- **ITEM-22** — verdict: CONCERN — **the load-bearing change, now solved correctly**: cleaned-char→box map in the shared extract path; the cleaned string output is unchanged; must keep the `&page[start..end]==content` invariant AND a parallel geometry array of equal length. Prototype the map alignment first.
- **ITEM-23** — verdict: CONCERN — new migration + storage derivative; per-page geometry can be large — store compactly (fraction i16/varint) and lazily.
- **ITEM-24** — verdict: PASS — backfill mirrors `file_rag::run_backfill`; idempotent; degrade-to-page-level until done.
- **ITEM-25** — verdict: PASS — endpoint reads stored geometry, `get_by_id_and_user` owner-scope, non-PDF → empty.
- **ITEM-26** — verdict: PASS — `createModule` mirror; nav order 15 free.
- **ITEM-27** — verdict: PASS — stores mirror `Projects`/`ProjectFiles`; live via `sync:file_index_state`; refetch on `sync:file` (external-delete count fix).
- **ITEM-28** — verdict: CONCERN — list state branches need `STATE_COVERAGE` (ITEM-41).
- **ITEM-29** — verdict: PASS — `Drawer`+Form mirror `ProjectFormDrawer`.
- **ITEM-30** — verdict: CONCERN — detail adds a direct search box (new interaction) + retrieval-mode line + indexing progress; multiple state branches → ITEM-41.
- **ITEM-31** — verdict: CONCERN — highest-risk UI item: folder upload + virtualization (2,000) + live per-doc status incl. no_text/failed-retry + batch-reject + dedup report + bulk-retry; many states → gallery coverage.
- **ITEM-32** — verdict: CONCERN — NEW chat frontend extension; `composeRequestFields`/`onConversationLoad` silent-failure risk; TEST-covered; responsive pill wrap.
- **ITEM-33** — verdict: PASS — picker mirrors MCP config modal.
- **ITEM-34** — verdict: PASS — project-extension mirrors `citations`.
- **ITEM-35** — verdict: CONCERN — `[n]` tokenizer must not double-tokenize literal brackets; render only when a `search_knowledge` result exists for the message.
- **ITEM-36** — verdict: CONCERN — `contentMatch` claims only `search_knowledge`, ordered before the file catch-all; empty + indexing-incomplete states.
- **ITEM-37** — verdict: PASS — additive-optional panel params; overlay reads geometry endpoint; graceful empty.
- **ITEM-38** — verdict: CONCERN — `lint:settings-field` (controls in `FormField`); mutual-exclusion; hub deep-link nudge.
- **ITEM-39** — verdict: CONCERN — mandatory both workspaces, sequenced before frontend; golden parity enforces.
- **ITEM-40** — verdict: PASS — desktop parity = mirror + `npm run check`.
- **ITEM-41** — verdict: CONCERN — exhaustive `STATE_COVERAGE` + stories incl. narrow-viewport, no_text, empty transparency panel, office fallback; compile-gated.
- **ITEM-42** — verdict: PASS — docs incl. the hub change.

No `BLOCKED` verdicts. Every prior-audit CRITICAL/HIGH is now an item with a test.
Remaining CONCERNs are handled by explicit items/tests/DECs (esp. ITEM-7 promotion
test, ITEM-12 real-worker emit test, ITEM-22 geometry prototype-first, ITEM-18
owner-scope/dedup tests, ITEM-10 seed lockstep).
