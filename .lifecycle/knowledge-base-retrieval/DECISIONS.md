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

### DEC-9: Reranker defaults + rollout. **(revised — see DEC-38 hub delivery)**
**Resolution:** `rerank_enabled` defaults **FALSE**, `rerank_candidate_k` defaults **30** (CHECK 1..200), `reranker_model_id` nullable/unset. The reranker model is **delivered through the hub** (DEC-38) — browse+download **BGE-reranker-v2-m3** like the embedding model — and surfaced by a discoverability nudge on the file-rag admin page, so it is NOT dark-by-default the way the audit warned.
**Basis:** user + codebase — the user chose hub delivery; mirrors `nomic-embed…` already in the hub; research candidate-N ≈ 20–50.

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

### DEC-14: Ingestion UX + scale cap. **(revised — status now from file_index_state)**
**Resolution:** Bulk multipart upload (each file → `ingest_bytes` → `spawn_index` → attach) plus attach-existing (which **reindexes if the file has 0 chunks**, DEC-39). Per-doc status comes from the new **`file_index_state`** table (DEC-39), NOT ambiguous chunk counts. `KB_MAX_DOCUMENTS = 2000`, atomic, 422 on overflow; **checksum dedup** (DEC-36); **server-side per-file size/type validation** (DEC-33).
**Basis:** codebase + audit — mirrors project bulk upload; the audit showed chunk-count alone can't express pending/indexing/failed/no-text.

### DEC-15: Orders + ids. **(corrected by audit)**
**Resolution:** chat-extension **order 23** (order 24 COLLIDES with `summarization/chat_extension` — verified `extension.rs:21 order:24`; 21 and 23 are the free slots before MCP=30); MCP id `Uuid::new_v5(NAMESPACE_URL, b"knowledge_base.ziee.internal")`; module init order 104; loopback route `/api/knowledge-base/mcp`.
**Basis:** codebase — order table re-verified after the audit caught the 24 collision; 23/104 unused; namespace unique.

### DEC-16: Desktop.
**Resolution:** Runs on the embedded desktop server (pgvector + local-runtime present — memory/file_rag run there); NOT added to `CORE_MODULE_BLOCKLIST`. Desktop needs only OpenAPI/TS regen + `npm run check`.
**Basis:** codebase — blocklist holds only `user-profile`/`server-update`.

### DEC-17: Permissions.
**Resolution:** `knowledge_base::use` (list/search/attach — granted to Users by migration 134) + `knowledge_base::manage` (create/edit/delete/upload); admins via `*`. Both tools read-only → `use`, approval-bypassed. Reranker admin config reuses the existing `file_rag::admin::manage`.
**Basis:** codebase — mirrors web_search/citations `use`+`manage` + grant-to-Users idiom.

### DEC-18: Naming.
**Resolution:** module `knowledge_base` (server) / `knowledge-base` (ui); route `/knowledge`; tool `search_knowledge`; capability `rerank`. No `ziee-chat` strings.
**Basis:** convention + `[[feedback_naming_ziee]]`.

### DEC-19: `document_count` maintenance. **(SUPERSEDED by DEC-32)**
**Resolution:** SUPERSEDED — see DEC-32. The count is **derived at read (`COUNT(*)`)**, not denormalized, because an external file delete cascade-removes the join row without an app-tx decrement → drift. Pinned by TEST-20 (external-delete no-drift) + TEST-21 (add/remove/re-add at boundary).
**Basis:** codebase audit — the denormalized counter drifts under `ON DELETE CASCADE`; a scoped `COUNT(*)` is cheap at ≤2000 rows.

### DEC-20: Retrieval transparency + grounded answers in v1.
**Resolution:** v1 ships (a) a "chunks used" transparency panel under any turn that called `search_knowledge`, and (b) a grounding instruction in the tool description + chat-extension note (answer only from results; say "not found"; cite the hit).
**Basis:** research — transparency + strict grounding are top trust drivers; opacity + untraceable synthesis are named anti-patterns. Low cost, high trust.

### DEC-21: Highlight geometry. **(SUPERSEDED by DEC-31 — user chose ingest-time)**
**Resolution:** SUPERSEDED by DEC-31. The audit proved the on-demand `page.text().search()` approach fails on nearly all real cleaned chunks (cleaned text has no positional map to the raw PDFium stream), so v1 uses **ingest-time geometry** instead.
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

### DEC-26: Per-document indexing feedback. **(revised — real backend, DEC-39)**
**Resolution:** Each doc row shows an **index-status badge** — `indexing` (warning + `Spin`) / `indexed` (success) / `failed` (destructive + Retry) / **`no_text`** (muted advisory, DEC-35) — read from the new **`file_index_state`** table (DEC-39), **live-updated via `sync:file_index_state`** (poll fallback via the indexing-status endpoint). A KB-level `Progress` bar shows on the Overview card while any doc indexes. Upload returns immediately; status streams in.
**Basis:** codebase audit — the original "derived from `file_chunks` counts + `sync:knowledge_base_document`" had NO backend: chunk counts can't distinguish pending/indexing/failed/no-text, and nothing emitted that event. DEC-39/Part I add the real state + emit source.

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

### DEC-31: Exact-passage highlight via INGEST-TIME geometry. **(user)**
**Resolution:** Capture per-character `tight_bounds` during PDF extraction and modify `clean_extracted_text` to emit a **parallel per-cleaned-char geometry array** (rotation-normalized fractions), so a chunk's cleaned `[start,end)` maps directly to boxes; persist per-page geometry (mirroring text-page storage) + backfill existing files; the `text-rects` endpoint reads STORED geometry (no live re-parse). Covers office docs (geometry captured from the temp PDF before deletion). Reliable exact box; page-level fallback only for files not yet backfilled / truly text-less.
**Basis:** user + codebase audit — the on-demand `page.text().search()` approach (old DEC-21) fails on ~all real cleaned chunks; ingest-time capture is the only precise, office-covering path.

### DEC-32: `document_count` derived at read.
**Resolution:** No denormalized column; `KnowledgeBase.document_count` (and the UI card) come from a scoped `COUNT(*)` over `knowledge_base_documents`. The detail store also refetches on `sync:file` so an external file delete updates the count live.
**Basis:** codebase audit — a denormalized counter drifts when a file is deleted through the file module (`ON DELETE CASCADE` bypasses the KB repo).

### DEC-33: Per-file size cap + server-side validation + itemized reject UX.
**Resolution:** Reuse the existing `MAX_FILE_SIZE = 50 MB` per file (revisit for KB later); **enforce size + type server-side** in the upload handler (the client `accept` is bypassable). When a batch has rejects, the toast **itemizes** which files and why (too-large / unsupported / duplicate), never a vague "some files failed". No total-KB-bytes cap in v1 (documented).
**Basis:** codebase — 50 MB is the current ingest cap; the audit flagged silent client-side rejection of N-of-500 as a real UX failure.

### DEC-34: In-flight upload behavior.
**Resolution:** Bulk upload is **N independent requests** with per-file client progress; navigating away cancels un-sent files (no resume in v1), and a re-drop is deduped (DEC-36) so recovery = re-drop. A "keep this tab open while uploading" hint shows during an active batch. Upload resume/recovery is roadmap.
**Basis:** convention — matches the existing per-file upload store; full resumability is out of v1 scope.

### DEC-35: Scanned / zero-text documents → `no_text` terminal state.
**Resolution:** A file that extracts to empty text (scanned/image PDF) is written `no_text` in `file_index_state` (distinct from `indexed`/`failed`), shown as a per-doc advisory badge and a KB-level "N documents have no extractable text (scanned?)" note. No OCR in v1 (roadmap).
**Basis:** codebase audit — `spawn_index` early-returns on no text and would otherwise look identical to "indexed", silently hiding un-retrievable docs.

### DEC-36: Checksum dedup on KB upload.
**Resolution:** KB upload/attach dedups by the `files.checksum` ingest already computes: a byte-identical file already in that KB is **skipped and reported** ("12 already in this KB"), not re-ingested as a new `file_id`. (Cross-KB the same file_id is simply reused.)
**Basis:** codebase audit — re-dropping a folder otherwise mints duplicate file_ids → duplicate chunks pollute retrieval/citations.

### DEC-37: Half-indexed KB honesty in chat.
**Resolution:** `search_knowledge` returns an `indexing_incomplete{searchable,total}` signal when the KB isn't fully indexed; the transparency panel shows an "indexing incomplete: S of T searchable" banner, and the grounding nudge is conditioned so the model won't claim a confident "not found" over an un-searched corpus.
**Basis:** codebase audit — background indexing means early queries silently search a subset; for a trust feature that must be surfaced.

### DEC-38: Reranker (and embedding) models delivered via the HUB. **(user)**
**Resolution:** Add `rerank: boolean` to the hub model-schema `capabilities` (both schema versions), add a **`bge-reranker-v2-m3-gguf`** hub manifest (`capabilities.rerank: true`), regen the hub index, and **mirror into the vendored seed** (two coordinated PRs: `ziee-ai/hub` + this repo; `SEED_HUB_VERSION` bumped in lockstep). The hub→llm_model capability map (`hub/handlers.rs:1612`) carries `rerank`. Discoverability = browse+download in the hub UI (like the existing `nomic-embed…` embedder) + a file-rag admin nudge. This reconciles the DEC-4/8/9 tension the audit raised (the reranker is now reachable, not dark).
**Basis:** user — chose hub delivery; mirrors the embedding model already in the hub.

### DEC-39: Index-status backend = `file_index_state` + sync emit (shared file_rag change).
**Resolution:** A `file_index_state` table (status pending/indexing/indexed/failed/no_text) written by `file_rag/ingest.rs` at each transition, emitting owner-scoped `sync:file_index_state`. This is the source of truth for per-doc status and the live stream; it's a shared `file_rag` addition (also usable by files_mcp/project surfaces). Attach-existing triggers a reindex when a file has 0 chunks.
**Basis:** codebase audit — the status UX had no backing state or event; this adds both minimally at the existing ingest transition points.

### DEC-40: Direct KB search box (verify outside chat).
**Resolution:** The KB detail page includes a search box that runs `search_knowledge` over that KB and renders the same hit rows + deep-link (reusing the chip/transparency components), so a scientist can verify "is my 2019 paper actually retrievable?" without opening a chat.
**Basis:** competitor research (NotebookLM/Onyx offer direct source search); reuses components already built for chat.

### DEC-41: Grounding is best-effort + cross-tool citation honesty.
**Resolution:** DEC-20 grounding is a prompt-level nudge, not enforcement; when other retrieval tools (web/lit/bio) are also attached, citation chips map ONLY `search_knowledge` hits, and the transparency panel states whether the turn used the KB ("this turn searched the knowledge base / did not"). No false implication that a web-sourced claim is KB-grounded.
**Basis:** codebase audit — the model retains parametric + other-tool knowledge; the trust UI must not conflate sources.

### DEC-42: File versioning under a KB.
**Resolution:** Retrieval + citations **pin the `version` the chunk was indexed from** (already carried on `file_chunks`/`SemanticHit`); re-uploading a new version re-indexes and re-captures geometry, and old citations resolve against their stored version so a highlight never lands on a shifted span. Deeper version-management UX is roadmap.
**Basis:** codebase — `file_chunks` already stores `version`/`blob_version_id`; DEC-31 geometry is keyed by `blob_version_id`.

Every decision is resolved. Headline user calls: DEC-5/6/7/8/23 (earlier) + DEC-31
(ingest-time highlight) + DEC-38 (hub-delivered reranker). DEC-19/21 are SUPERSEDED
(by DEC-32/DEC-31); DEC-9/14/15/26 revised per the audit. No open question remains
for implementation; the roadmap (OCR, versioning UX, upload resume, structure-aware
ingest) is explicitly deferred.

### DEC-43: Where do the promoted retrieval-limit constants live, and which are exposed?
**Resolution:** all four (`kb_max_documents`, `search_max_hit_chars`, `search_snippet_chars`, `search_max_top_k`) become columns on the existing `file_rag_admin_settings` singleton (the shared Document-RAG admin surface the KB module already reuses), exposed via a `RetrievalLimitsSection` card. The PDF-geometry line-merge tolerance (`0.012`) stays a named const — it is an internal rendering heuristic, not a deployment policy.
**Basis:** convention — mirrors the reranker columns added to the same row (migration 135); KB reuses file_rag retrieval + its admin settings rather than a separate settings surface.
