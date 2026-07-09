# PLAN_AUDIT — knowledge-base-retrieval

Audited against the codebase (base = origin/main @ 9a6fb88c6, highest migration
132). Backend surfaces (reranker, highlight) and the UI conventions (kit, gallery,
chat registry) were mapped end-to-end before this audit.

## Breakage risk

- **Reranker additive + gated** — `grep rerank` = 0; trait default-impl keeps every
  caller compiling; proxy is an explicit allowlist (new route only); the retrieval
  rerank stage is gated `rerank_enabled` (default FALSE) so `semantic_search`
  (incl. `files_mcp`) is byte-identical until opt-in; rerank error → pre-rerank order.
- **KB reuse of `semantic_search`** is call-only; `file_chunks` untouched; KB grouping is pure additive tables.
- **Chat integration is registry-based, not core edits** — KB attach + citation
  chip + transparency panel register into the chat extension registry / streamdown
  component map; the ONE core edit is extending `PanelRendererMap['file']` with
  optional `{page,charRange}` (additive optional fields — existing `{fileId,version}`
  callers unaffected).
- **PDF viewer overlay** wraps the existing `<img>` in a `relative` container; the
  `%`-box needs no pixel measurement (`object-contain`); non-citation views render
  unchanged; the scroll-to-page effect is opt-in on the new `page` param.
- **`lint:settings-field` risk** — the reranker admin controls (ITEM-32) live in a
  settings `sections/` file, so every control MUST be inside `FormField`/`Field` or
  the build fails; called out in the item.
- **Gallery state-matrix is compile-gated** — every new loading/empty/error/overlay
  branch MUST get a `STATE_COVERAGE` entry or `tsc`/`check:state-matrix` fails; a
  standing risk on ITEM-18/21/22/24/27/29, owned by ITEM-35.
- **Overlay stacking / tooltip flicker** — opening the file viewer from within the
  KB drawer, and Confirm+Tooltip on card rows, hit documented gotchas; mitigated by
  the higher-layer-open guard + single-sibling-Tooltip + controlled-Confirm patterns.
- **2,000-doc list perf** — rendering 2,000 `FileCard`s would jank; mitigated by
  virtualized/paged list (ITEM-22).
- Chat-extension order 24 free; MCP id namespace unique; nav order 15 free (Chats 10 / Projects 20).

## Pattern conformance

- Reranker mirrors the embedding capability at every point.
- KB backend mirrors `web_search`/`citations`/`project`/`project_files`.
- Every UI surface names a concrete reference to mirror (list/card/detail/form/
  upload/settings/project-extension) and consumes the kit + tokens + state trio; chat
  surfaces mirror the MCP/memory composer + `useStreamdownComponents`/`McpToolCallUI`
  precedents. Highlight overlay conforms to the page-image viewer + `get_preview` gating.

## Migration collisions

- Highest = 132. New: 133 (KB tables), 134 (KB grant), 135 (file_rag reranker ALTER). No collision; all additive; no new `CREATE EXTENSION vector`. `cargo clean` after adding migrations (build-DB note).

## OpenAPI regen

- New REST types (KB CRUD/documents/attachments/indexing-status, `Rerank*`/capability, `text-rects`) + `SyncEntity` variants ⇒ `just openapi-regen` REQUIRED for BOTH `ui` and `desktop/ui` (ITEM-33); golden `emit_ts` enforces. JSON-RPC `/mcp` + local-runtime `/rerank` are plain routes, excluded from OpenAPI.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — DTOs + trait default + one OpenAI body; mirrors `embeddings`.
- **ITEM-2** — verdict: PASS — JSONB field (no migration) + allowlist + guard.
- **ITEM-3** — verdict: PASS — `dispatch::rerank` mirrors `embed`.
- **ITEM-4** — verdict: CONCERN — three coordinated edits (argv/inject/proxy); `--pooling rank` pairing verified at impl (DEC-10). No blocker.
- **ITEM-5** — verdict: PASS — `ADD COLUMN` mirrors migration 99; 135 free.
- **ITEM-6** — verdict: CONCERN — compile-checked positional `query_as!` needs SET+RETURNING per column; compiler + TEST guard.
- **ITEM-7** — verdict: CONCERN — core behavioral change; preserve existing guards + fallback; gated OFF by default.
- **ITEM-8** — verdict: PASS — additive tables mirroring `project_files`/`project_bibliography`.
- **ITEM-9** — verdict: PASS — idempotent grant mirrors 104.
- **ITEM-10** — verdict: PASS — module entry + loopback upsert mirror `web_search`; order 104 free.
- **ITEM-11** — verdict: CONCERN — new REST types ⇒ `just openapi-regen` (ITEM-33).
- **ITEM-12** — verdict: PASS — CRUD/membership mirror `project`+`file/project_extension`; `resolve_scope_file_ids` a plain owner-scoped SELECT.
- **ITEM-13** — verdict: PASS — cap idiom mirrors `attach_file_capped`; 422 mirrors project upload; bulk ingest reuses `ingest_bytes`.
- **ITEM-14** — verdict: PASS — tool dispatch mirrors `citations`; retrieval is the reranked `semantic_search`.
- **ITEM-15** — verdict: CONCERN — the two `mcp.rs` edits are a silent-failure point; unit TEST guards.
- **ITEM-16** — verdict: PASS — `createModule` routes+`sidebarNavigation` mirror `projects/module.tsx`; nav order 15 free; desktop auto-discovers.
- **ITEM-17** — verdict: PASS — stores mirror `Projects`/`ProjectFiles`; sync self-gate mirrors the McpServer rule; live status via `sync:knowledge_base_document`.
- **ITEM-18** — verdict: CONCERN — list-page state branches (loading/error/empty) each need a `STATE_COVERAGE` entry (ITEM-35); layout mirrors `ProjectsListPage`.
- **ITEM-19** — verdict: PASS — `KnowledgeBaseCard` mirrors `ProjectCard`; controlled `Confirm` avoids tooltip-flicker; status tones use semantic tokens.
- **ITEM-20** — verdict: PASS — `Drawer`+`Form`/`FormField`+zod mirror `ProjectFormDrawer`; Save hidden without perm.
- **ITEM-21** — verdict: CONCERN — detail state branches (loading/not-found/indexing-progress/empty "Used in") need `STATE_COVERAGE` (ITEM-35); layout mirrors `ProjectDetailPage`.
- **ITEM-22** — verdict: CONCERN — the highest-risk UI item: bulk folder upload + drag-drop + per-doc live status + **virtualization for 2,000 rows** + multi-select. Mirrors `ProjectFilesManagePanel`/`FileCard` but adds index-status + scale; every state (upload/indexing/failed/empty/error) needs gallery coverage. Broken into explicit sub-states in TESTS.
- **ITEM-23** — verdict: CONCERN — a NEW chat frontend extension; must correctly `composeRequestFields` (send kb ids) + `onConversationLoad` hydrate + persist; mirrors MCP/memory composer exactly. Silent-failure risk if `composeRequestFields` omitted — TEST-covered.
- **ITEM-24** — verdict: PASS — picker mirrors the MCP config modal (always-mounted `input_area_suffix`); empty-state links to `/knowledge`.
- **ITEM-25** — verdict: PASS — project-extension mirrors `citations/project-extension` (`knowledge_kinds` slot).
- **ITEM-26** — verdict: CONCERN — `[n]` tokenizer + streamdown override is per-`content.id` scoped; the footnote/blockquote overrides are the working precedent but markdown post-processing is fiddly (must not double-tokenize real bracket text). Bounded by rendering only when a `search_knowledge` result exists for the message.
- **ITEM-27** — verdict: CONCERN — `tool_result` `contentMatch` must claim ONLY `search_knowledge` blocks and render before the file catch-all (ordering per the registry's first-match rule); model on `McpToolCallUI`.
- **ITEM-28** — verdict: PASS — additive optional `{page,charRange}` on `PanelRendererMap['file']` + `FilePreviewDrawer.openPreview`; existing callers unaffected.
- **ITEM-29** — verdict: CONCERN — `%`-overlay needs the landscape-rotation transform (the one geometry special case) + scroll-to-page (net-new to `PdfBody`); graceful empty-rects fallback bounds risk.
- **ITEM-30** — verdict: CONCERN — **load-bearing feasibility**: cleaned-text offsets have no direct PDFium map; `page.text().search()` relocation is best-effort/empty-on-no-match. Prototype-first; graceful fallback (ITEM-29) + DEC-21.
- **ITEM-31** — verdict: PASS — endpoint mirrors `get_preview` gating; `spawn_blocking`; new type ⇒ ITEM-33.
- **ITEM-32** — verdict: CONCERN — `lint:settings-field` requires every control inside `FormField`; capability toggle mirrors `text_embedding` mutual-exclusion.
- **ITEM-33** — verdict: CONCERN — mechanical but mandatory in BOTH workspaces; golden parity test enforces.
- **ITEM-34** — verdict: PASS — desktop parity = mirror + `npm run check`; pgvector/local-runtime proven on desktop.
- **ITEM-35** — verdict: CONCERN — the gate-satisfaction item: `STATE_COVERAGE` entries + `GalleryStory` fixtures for every new surface/state, runtime-health zero-HIGH, Layer-A axe. Compile-gated; must be exhaustive.
- **ITEM-36** — verdict: PASS — docs-only; mirrors existing CLAUDE.md sections.

No `BLOCKED` verdicts. `CONCERN`s are handled by explicit items/tests/DECs:
ITEM-4 (`--pooling` verify → DEC-10), ITEM-6/7 (compiler+fallback+TEST), ITEM-11/33
(regen), ITEM-15/23 (wiring silent-failure → TEST), ITEM-18/21/22/35 (gallery
state-matrix + virtualization → DEC-24/25 + TEST), ITEM-26/27 (chat-render ordering
→ TEST), ITEM-29/30 (highlight feasibility → DEC-21 + graceful fallback), ITEM-32
(`lint:settings-field`).
