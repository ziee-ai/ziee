# PLAN_AUDIT — knowledge-base-retrieval

Audited against the codebase (worktree base = origin/main @ 9a6fb88c6, highest
migration `00000000000132_add_openrouter_provider_type.sql`). Surface for the
reranker (Part R) and the highlight overlay (Part C) was mapped end-to-end before
this audit; findings below reflect that trace.

## Breakage risk

- **Reranker is additive at every seam.** `grep -rni rerank src-app/` = 0 hits;
  the trait method ships with a **default "unsupported" impl** so gemini/anthropic
  and every existing caller compile unchanged. The proxy is an **explicit
  allowlist** (`proxy_router.rs` mounts exactly 3 routes) — a new `/rerank` route
  is purely additive, no wildcard behavior changes.
- **`file_rag` retrieval change is gated.** The rerank stage runs only when
  `rerank_enabled && reranker_model_id.is_some()`; default `rerank_enabled=FALSE`
  ⇒ existing `semantic_search` behavior (incl. the `files_mcp` tool) is
  byte-identical until an admin opts in. On rerank error it falls back to the
  pre-rerank order (no new failure mode).
- **`ModelCapabilities.rerank` is JSONB** — no migration, no default backfill;
  absent key = `None` = not a reranker. No existing model row changes meaning.
- **KB reuse of `semantic_search` is call-only** (verified signature
  `(&[Uuid], Uuid, &str, i64, &FileRagAdminSettings)`), adds a second caller,
  breaks nothing. `file_chunks` schema untouched by Part K.
- **Highlight endpoint is read-only + additive** (new route, `FilesRead` gate);
  the viewer overlay wraps the existing `<img>` without changing its layout
  (`w-full object-contain`), so non-citation views render unchanged.
- **Chat-extension order 24** is free (8/10/15/20/22/25/26/27/28/29 taken), runs
  before MCP=30. New MCP id namespace is unique. No `mcp_servers` collision.
- **Denormalized `document_count`** risks drift — mitigated by tx-consistent
  updates (repository invariant, TEST-covered).
- **Reranker candidate expansion cost**: pulling `rerank_candidate_k` (≤200) rows
  then a cross-encoder pass adds latency; bounded by the CHECK + only on opt-in.

## Pattern conformance

- Reranker mirrors the embedding capability at **every** point (provider/DTO/
  dispatcher/local-flag/proxy/settings) — the tightest possible conformance, one
  proven analog per file.
- KB module/MCP/collection/permissions/sync/frontend conform to
  `web_search`+`citations`+`project`+`project_files`+`projects` as before.
- Highlight overlay conforms to the existing page-image viewer + `get_preview`
  endpoint gating; geometry uses the vendored pdfium-render API already in use.

## Migration collisions

- Highest existing = `132`. New: **133** (KB tables), **134** (KB grant), **135**
  (file_rag reranker ALTER). No collision; all additive (new tables; idempotent
  grant; `ADD COLUMN ... DEFAULT`). No new `CREATE EXTENSION vector` (chunks stay
  in `file_chunks`). `cargo clean` after adding migrations (documented build-DB
  behavior) — an implementation note, not an item.

## OpenAPI regen

- New REST types across three areas — KB CRUD/documents, the `Rerank*`/capability
  fields on model + file-rag settings, and the `text-rects` response — each derive
  `schemars::JsonSchema` and are declared in `*_docs` builders ⇒ **`just
  openapi-regen` REQUIRED for BOTH `ui` and `desktop/ui`** (ITEM-28). The
  `openapi::emit_ts` golden test enforces regen ran. `SyncEntity` variants
  regenerate the TS union in both workspaces. The `/…/mcp` JSON-RPC route and the
  local-runtime `/rerank` proxy route are plain `.route`s excluded from OpenAPI.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — additive DTOs + trait default-impl + one real OpenAI body; mirrors `embeddings` at `openai.rs:946`.
- **ITEM-2** — verdict: PASS — JSONB field (no migration) + allowlist string + a guard fn mirroring `embedding_unsupported_reason`.
- **ITEM-3** — verdict: PASS — `dispatch::rerank` mirrors `dispatch::embed` exactly; shared home already imported by `file_rag`.
- **ITEM-4** — verdict: CONCERN — the proxy allowlist + argv + auto_start injection are three coordinated edits; `--pooling rank` may be required alongside `--reranking` — verify against `llama-server --help` at implementation (a DEC). No blocker.
- **ITEM-5** — verdict: PASS — `ADD COLUMN` mirrors migration 99's `embedding_model_id`; number 135 free.
- **ITEM-6** — verdict: CONCERN — the positional `query_as!` macro in `update_admin_settings` is compile-checked; every new column needs a SET **and** a RETURNING line or it won't compile — easy to half-edit. Mitigated by the compiler + TEST.
- **ITEM-7** — verdict: CONCERN — the core behavioral change; must preserve the empty-scope/empty-query guards and the embed-failure fallbacks already in `semantic_search`. Requires `just openapi-regen`? No (no type change). Gated + fallback-safe.
- **ITEM-8** — verdict: PASS — capability toggle mirrors `text_embedding`'s mutually-exclusive UX; admin dropdown mirrors the embedding-model picker.
- **ITEM-9** — verdict: PASS — additive tables mirroring `project_files`/`project_bibliography`.
- **ITEM-10** — verdict: PASS — idempotent grant mirrors migration 104; strings cross-checked with ITEM-12.
- **ITEM-11** — verdict: PASS — module entry + loopback upsert mirror `web_search/mod.rs`; init order 104 free.
- **ITEM-12** — verdict: PASS — `PermissionCheck` impls mirror `web_search/permissions.rs`; test pins strings.
- **ITEM-13** — verdict: CONCERN — new REST types ⇒ requires `just openapi-regen` (ITEM-28); resolved by the explicit dependency.
- **ITEM-14** — verdict: PASS — CRUD/membership mirror `project`+`file/project_extension`; `resolve_scope_file_ids` is a plain owner-scoped SELECT.
- **ITEM-15** — verdict: PASS — cap idiom mirrors `attach_file_capped`; 422 mirrors project upload; ingest reuses `ingest_bytes`.
- **ITEM-16** — verdict: PASS — tool dispatch mirrors `citations`/`files_mcp`; retrieval is the reranked `semantic_search` call.
- **ITEM-17** — verdict: CONCERN — the two `mcp.rs` edits are a documented silent-failure point; mitigated by a dedicated unit TEST.
- **ITEM-18** — verdict: PASS — module registration mirrors `projects/module.tsx`; desktop auto-discovers.
- **ITEM-19** — verdict: PASS — list page/store mirror `Projects.store.ts`; sync self-gate mirrors the McpServer rule.
- **ITEM-20** — verdict: PASS — upload/list/cap panel mirrors `ProjectFilesManagePanel`; per-doc status derives from `file_chunks` counts (ITEM-14).
- **ITEM-21** — verdict: PASS — attachment picker mirrors conversation-scoped pickers; reads generated types.
- **ITEM-22** — verdict: PASS — project-extension mirrors `citations/project-extension` (`knowledge_kinds`).
- **ITEM-23** — verdict: PASS — chips + transparency render over the tool result's `structuredContent`; no backend change.
- **ITEM-24** — verdict: CONCERN — **load-bearing feasibility risk**: stored offsets are into *cleaned* text (dedup/whitespace-collapse/reflow) with no positional map to PDFium chars; relocation via `page.text().search(content)` is best-effort and returns empty on no-match. Bounded by the graceful page-level fallback (ITEM-27) and a DEC that fixes the mechanism + accepts partial coverage. Prototype-first at implementation.
- **ITEM-25** — verdict: PASS — endpoint mirrors `get_preview` gating; runs ITEM-24 in `spawn_blocking`; new response type ⇒ ITEM-28.
- **ITEM-26** — verdict: PASS — `%`-based overlay needs no pixel measurement (`object-contain`); landscape rotation transform is the one special case (from the geometry map).
- **ITEM-27** — verdict: PASS — deep-link + graceful fallback; extends `FilePreviewDrawer` state.
- **ITEM-28** — verdict: CONCERN — mechanical but mandatory in BOTH workspaces; golden `emit_ts` parity test fails the build if skipped.
- **ITEM-29** — verdict: PASS — desktop parity = regen + `npm run check`; pgvector + local-runtime proven on desktop.
- **ITEM-30** — verdict: CONCERN — new render states MUST have gallery cells or `check:state-matrix` fails phase 8; budgeted as its own item.
- **ITEM-31** — verdict: PASS — docs-only; mirrors existing CLAUDE.md module sections.

No `BLOCKED` verdicts. `CONCERN`s are handled by explicit items/tests/DECs:
ITEM-4 (`--pooling` verify → DEC-6), ITEM-6/7 (compile-checked + fallback + TEST),
ITEM-13/28 (regen dependency), ITEM-17 (wiring TEST), ITEM-24 (feasibility →
DEC-5 mechanism + graceful fallback), ITEM-30 (gallery).
