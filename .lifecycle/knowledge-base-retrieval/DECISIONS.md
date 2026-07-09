# DECISIONS — knowledge-base-retrieval

Every input the implementation needs, resolved up front. Four headline calls were
made by the user via AskUserQuestion (DEC-5/6/7/8, marked **(user)**); the rest are
resolved by codebase convention. No unresolved markers remain.

### DEC-1: Extend `file_rag`, or a new `knowledge_base` module?
**Resolution:** New `knowledge_base` module — a collection + scoping + agent-tool layer reusing `file_rag`'s retrieval engine and the `file` ingest. A KB is a named owner-scoped set of `file_id`s; retrieval resolves KB → file_ids → the existing `file_rag::retrieval::semantic_search`.
**Basis:** codebase — `semantic_search` already takes arbitrary `scope_ids`; `citations` set the precedent of a thin module reusing another's engine.

### DEC-2: Data model.
**Resolution:** `knowledge_bases` → `knowledge_base_documents` (M:N to `files`); chunks/embeddings reuse the existing `file_chunks` table; attachment via `conversation_knowledge_bases` + `project_knowledge_bases`. No per-KB chunk partition.
**Basis:** codebase — mirrors `project_files`/`project_bibliography`; `file_chunks` already keys chunks by `file_id` with full provenance.

### DEC-3: Chunking.
**Resolution:** Reuse `file_rag`'s page-aware char-window chunker (1200/200, admin-tunable) as-is. Structure-aware/contextual chunking is out of scope (roadmap).
**Basis:** codebase + research — chunking is shared; changing it would re-index all `file_rag` consumers. Structural chunking depends on the deferred parser upgrade.

### DEC-4: Embedding model — shared or per-KB? **(user-confirmed: single shared)**
**Resolution:** All KBs share the single deployment-wide `file_rag` embedding model (`file_rag_admin_settings.embedding_model_id`) — one embedding space. No per-KB model; no new settings singleton (KB inherits file_rag admin config).
**Basis:** user + codebase + research — `file_chunks.embedding` is one fixed-dimension `halfvec` column with one HNSW index; mixing embedding spaces is impossible. Research §4: a strong general model + hybrid + reranker beats a domain embedder, so the domain budget goes to the reranker.

### DEC-5: How the agent uses it. **(user)**
**Resolution:** On-demand MCP tool `search_knowledge`, auto-attached when ≥1 KB is bound to the conversation; **no `before_llm_call` chunk injection**. The chat extension only sets the attach flag + a one-line note listing available KBs + the grounding nudge.
**Basis:** user + research §7 — Anthropic "just-in-time context loading"; every recent built-in exposes retrieval as a tool; injecting a 500-doc library every turn is the anti-pattern this feature removes.

### DEC-6: Scoping. **(user: user-owned only in v1)**
**Resolution:** v1 = user-owned KBs (owner-scoped like `file_chunks`), attachable to the owner's conversations and projects. Shared/org/admin KBs are out of scope (roadmap) — they need cross-user `file_chunks` read, a cross-cutting RBAC change.
**Basis:** user + codebase — `file_chunks.user_id` + `semantic_search`'s `user_id` filter make retrieval strictly owner-scoped today.

### DEC-7: Citation precision. **(user: exact-passage highlight overlay in v1)**
**Resolution:** Ship the highlight overlay in v1: numbered citation chips (hover preview) → open `FilePreviewDrawer` at the source page → draw a `%`-positioned box over the exact passage. **Mechanism (DEC-21):** best-effort — a new endpoint relocates the chunk's content on the raw PDF via `page.text().search()` and returns fraction rects; when the search fails or the file is non-PDF, the chip **gracefully degrades to page-level** deep-link (no box). Also ship the retrieval-transparency panel (DEC-20).
**Basis:** user + research — click-citation→exact-passage is the #1 trust interaction (NotebookLM); "filename/page-only" is a named anti-pattern.

### DEC-8: Reranker in v1, and how hosted. **(user: yes; self-hosted cross-encoder)**
**Resolution:** v1 adds a self-hosted cross-encoder reranker served by ziee's local engine (llama.cpp `--reranking`), as a new `rerank` model capability mirroring the embedding capability end-to-end. Retrieval expands to a candidate pool, calls the reranker, reorders, truncates to top-k. Fully air-gap-capable; no external egress.
**Basis:** user + research §3 — the reranker (not the base embedder) is where domain specialization earns its keep; llama-server supports `/rerank`.

### DEC-9: Reranker defaults + rollout.
**Resolution:** `rerank_enabled` defaults **FALSE**, `rerank_candidate_k` defaults **30** (CHECK 1..200), `reranker_model_id` nullable/unset. **No reranker GGUF is bundled** — the admin marks a model with the `rerank` capability and selects it (mirrors the embedding-model picker; ziee ships no embedder either). Recommended model in docs: **BGE-reranker-v2-m3** (MIT); MedCPT (biomedical) is a documented A/B.
**Basis:** convention — matches `file_rag`'s "no embedding model configured by default → feature dormant until an admin sets one" posture; research candidate-N ≈ 20–50.

### DEC-10: Reranker serving flags.
**Resolution:** `llamacpp_argv` emits `--reranking` **and** `--pooling rank` when the model's `rerank` capability is set; the same-port proxy forwards `POST /v1/rerank`. Verify the exact flag pair against `llama-server --help` at implementation (llama.cpp couples reranking to rank-pooling).
**Basis:** codebase + external — mirrors the `--embeddings` capability→flag→proxy wiring; llama.cpp requires rank pooling for reranking.

### DEC-11: Where the reranker lives (ownership).
**Resolution:** The `rerank` capability is a **shared** cross-cutting addition: `ai-providers` (trait+DTO+OpenAI impl), `llm_model` capabilities, `memory::engine::dispatch::rerank`, `llm_local_runtime` (argv/auto_start/proxy), and the `file_rag` retrieval+settings. It is NOT owned by the `knowledge_base` module, and it also upgrades the existing `files_mcp` `semantic_search` (any `semantic_search` caller gets reranking once an admin enables it).
**Basis:** codebase — `dispatch` and `file_rag::retrieval` are the shared homes already imported across modules; keeping rerank there avoids KB-local duplication.

### DEC-12: Rerank placement in retrieval.
**Resolution:** The rerank stage lives inside `file_rag::retrieval::semantic_search` (candidate-pool → `dispatch::rerank` → reorder → `truncate(top_k)`), so the `files_mcp` and KB callers are untouched. Preserve the existing empty-scope/empty-query guards and embed-failure fallbacks; on rerank error keep pre-rerank order.
**Basis:** codebase — the truncation point in `retrieval.rs` is the natural seam; keeps callers stable.

### DEC-13: Connected-vs-airgapped.
**Resolution:** Inherit `file_rag`'s degradation: no embedding model → FTS-only; reranker is optional and OFF by default. Every layer works offline once the (optional) local embedding + rerank GGUFs are present.
**Basis:** codebase + research — `plan_arm` already collapses to FTS when no model is set; the reranker is self-hosted.

### DEC-14: Ingestion UX + scale cap.
**Resolution:** Bulk multipart upload to a KB (each file → `file::ingest::ingest_bytes` → `file_rag` `spawn_index` → attach) plus attach-existing-file_ids. Per-doc `index_status` from `file_chunks` counts; boot backfill self-heals. `KB_MAX_DOCUMENTS = 2000` (≥ the 500 bar; HNSW handles incremental inserts), atomic, 422 on overflow.
**Basis:** codebase + research §8 — mirrors project bulk upload + `attach_file_capped`; pgvector+HNSW is comfortable at this scale.

### DEC-15: Orders + ids.
**Resolution:** chat-extension order 24 (free, before MCP=30); MCP id `Uuid::new_v5(NAMESPACE_URL, b"knowledge_base.ziee.internal")`; module init order 104; loopback route `/api/knowledge-base/mcp`.
**Basis:** codebase — order table verified; 24/104 unused; namespace unique.

### DEC-16: Desktop.
**Resolution:** Runs on the embedded desktop server (pgvector + local-runtime present — memory/file_rag run there); NOT added to `CORE_MODULE_BLOCKLIST`. Desktop needs only OpenAPI/TS regen + `npm run check`.
**Basis:** codebase — blocklist holds only `user-profile`/`server-update`.

### DEC-17: Permissions.
**Resolution:** `knowledge_base::use` (list/search/attach — granted to Users by migration 134) + `knowledge_base::manage` (create/edit/delete/upload); admins via `*`. Both tools read-only → `use`, approval-bypassed. Reranker admin config reuses the existing `file_rag::admin::manage`.
**Basis:** codebase — mirrors web_search/citations `use`+`manage` + grant-to-Users idiom.

### DEC-18: Naming.
**Resolution:** module `knowledge_base` (server) / `knowledge-base` (ui); route `/knowledge`; tool `search_knowledge`; capability `rerank`. No `ziee-chat` strings.
**Basis:** convention + `[[feedback_naming_ziee]]`.

### DEC-19: `document_count` maintenance.
**Resolution:** Denormalized on `knowledge_bases`, updated in the same transaction as attach/remove; a repository invariant test guards it.
**Basis:** convention — keeps list views cheap at 2000 docs/KB; correctness pinned by TEST-17.

### DEC-20: Retrieval transparency + grounded answers in v1.
**Resolution:** v1 ships (a) a "chunks used" transparency panel under any turn that called `search_knowledge`, and (b) a grounding instruction in the tool description + chat-extension note (answer only from results; say "not found"; cite the hit).
**Basis:** research — transparency + strict grounding are top trust drivers; opacity + untraceable synthesis are named anti-patterns. Low cost, high trust.

### DEC-21: Highlight geometry — on-demand vs ingest-time.
**Resolution:** v1 = **on-demand** endpoint that re-parses the PDF and relocates the span via `page.text().search()` (PDF-only, best-effort, empty-on-no-match). Ingest-time geometry capture (capture char boxes in `PdfProcessor::extract_text` before `clean_extracted_text`, sidecar + backfill) — which also covers office docs and is precise — is deferred to v1.5 (roadmap). The load-bearing alignment routine (cleaned-text span ↔ raw PDFium chars) is prototyped first at implementation.
**Basis:** codebase — stored offsets are into cleaned text with no positional map to PDFium chars; on-demand avoids a shared-extraction-path change + migration + backfill for v1, and the graceful fallback (DEC-7) bounds the risk. Office coverage + precision is the recorded upgrade.

### DEC-22: Reranker as a large shared sub-feature — split or inline?
**Resolution:** Planned inline as a dependency of this feature (the KB's quality bar needs it), but structured as its own item cluster (Part R) so it could be landed/reviewed as a distinct commit range. It is not gated behind the KB module and independently benefits `files_mcp`.
**Basis:** convention — it mirrors the embedding capability precisely and is additive/opt-in; splitting into a separate lifecycle is optional, not required for correctness.

### DEC-23: KB ↔ project relationship — standalone-reusable vs project-owned. **(user: standalone-reusable)**
**Resolution:** KBs are a **standalone, reusable library** owned by the user (managed at `/knowledge`); a project or a chat **attaches** the KBs it needs (many-to-many via `project_knowledge_bases` / `conversation_knowledge_bases`). The same KB can be attached to multiple projects and chats; a chat can pull from multiple KBs. This is distinct from — and coexists with — the existing per-project *files* (the raw-prepend `project_files` path), which KB retrieval supersedes for scale. Project-owned (1:1, KB-inside-project) is explicitly NOT the model.
**Basis:** user + codebase — already the data model in DEC-2 / ITEM-9 (join tables, not a project FK) / ITEM-22 (project-extension attaches, does not own). Confirmed by the user over the project-owned and "both" alternatives.

### DEC-24: Information architecture — where the KB lives.
**Resolution:** A top-level **"Knowledge"** sidebar nav entry (order 15, between Chats=10 and Projects=20), owned by the new `knowledge-base` UI module, routing `/knowledge` (list) + `/knowledge/:id` (detail). Reranker/embedding config stays on the existing **file-rag admin settings** page (not a KB page). KB attach to a chat lives in the **composer**, KB attach to a project lives in the project's **"Knowledge bases" knowledge-kind** — mirroring how files/references already attach.
**Basis:** codebase — mirrors `projects/module.tsx` nav registration + the `settings`/`knowledge_kinds` split; keeps the retrieval-engine admin where it already is (file-rag), and the collection UX where users manage content.

### DEC-25: Documents list at scale (2,000 docs).
**Resolution:** The documents list is **virtualized / paginated** (kit `Table virtualized` or `List` + Load-More), not 2,000 mounted `FileCard`s. First-load shows a `Spin`; background refreshes patch rows in place (no blink).
**Basis:** codebase + perf — `ProjectFilesManagePanel` renders ≤100 cards un-virtualized; at 2,000 that janks, so the KB panel uses the kit's virtualization (already used by `Table`).

### DEC-26: Per-document indexing feedback.
**Resolution:** Each doc row shows an **index-status badge** — `indexing` (warning tone + tiny `Spin`), `indexed` (success), `failed` (destructive + Retry) — derived from `file_chunks` counts and **live-updated via `sync:knowledge_base_document`** (poll fallback). A KB-level `Progress` bar appears on the detail Overview card only while any doc is `indexing`. No blocking modal during ingest — upload returns immediately and status streams in.
**Basis:** codebase + research/competitor UX — per-source ingest status is a top "legibility" pattern (NotebookLM/Open WebUI); ziee already streams status via sync; `file_rag` indexes in the background.

### DEC-27: KB attach affordance = composer chips, not a drawer.
**Resolution:** Attaching KBs to a conversation is a composer `+`-menu item ("Attach knowledge base") + a **"Knowledge · N" status pill** with per-KB remove; project-inherited KBs render as distinct **read-only** chips so the active retrieval scope is always visible. No dedicated per-conversation settings drawer (ziee has none).
**Basis:** codebase — mirrors the memory-mode pill + MCP status row exactly; competitor research flags rigid workspace-only scoping as an anti-pattern, so per-chat composability is deliberate.

### DEC-28: Citation chip + transparency presentation.
**Resolution:** Numbered inline chips `[1] [2]` (kit `Tag`/`Badge`, info tone, focusable) with a hover/focus `Popover` preview (file · page · snippet); click/Enter opens the source. A **retrieval-transparency panel** ("Searched K KBs · M chunks") renders per `search_knowledge` result, **default collapsed**, expandable to the chunk list; empty result shows "No matching passages found". Chips inject via a `[n]` streamdown tokenizer + `useStreamdownComponents` override; the panel is a `tool_result` renderer with a `contentMatch` claiming only `search_knowledge` blocks.
**Basis:** codebase + competitor research — Perplexity-style numbered chips + NotebookLM-style click-to-source + Onyx-style retrieval transparency; the streamdown footnote/blockquote overrides + `McpToolCallUI` are the working precedents.

### DEC-29: In-chat citation opens the RIGHT PANEL (not the global drawer).
**Resolution:** In a conversation, a citation opens the **chat right panel** via `displayInRightPanel({type:'file', data:{fileId, version, page, charRange}})`; this requires extending `PanelRendererMap['file']` with optional `{page, charRange}` and threading them to the PDF viewer. Outside chat (KB detail "view document") the same params flow through the global `FilePreviewDrawer.openPreview`.
**Basis:** codebase — in chat the file viewer IS the right panel (`InlineFilePreview.handleOpenInPanel`); the global drawer is the non-chat surface. Additive optional fields keep existing callers intact.

### DEC-30: Folder-scale ingest affordance.
**Resolution:** The documents panel uses the kit `Upload` with **`directory` (folder pick) + `multiple`** plus a drag-drop overlay, so a user can drop a folder of 500 PDFs. `accept` limits to text-extractable types; oversize/over-cap rejected pre-flight with a toast (mirrors `ProjectFilesManagePanel`).
**Basis:** codebase — kit `Upload` supports `directory`; mirrors the existing bulk-upload panel; matches the "point at a folder of 500 PDFs" goal.

Every decision above is resolved. The five headline calls (DEC-5/6/7/8/23) are
user-confirmed; DEC-24–30 (added this replan) resolve the UI/UX by codebase
convention and carry no new open question. Two convention-resolved points worth the user's awareness (not
blockers): **no reranker GGUF is bundled** — an admin must supply/select one
before rerank does anything (DEC-9); and the v1 highlight overlay is **PDF-only,
best-effort**, degrading to page-level for office docs / failed matches until the
ingest-time geometry upgrade (DEC-21).
