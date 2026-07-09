# DECISIONS — knowledge-base-retrieval

Every input the implementation needs, resolved up front. Decisions marked
**(confirm with user)** are the highest-leverage product calls — resolved here by
convention so implementation can proceed, but surfaced to the user in the halt
message for override before any code is written.

### DEC-1: Extend `file_rag`, or a new `knowledge_base` entity/module?
**Resolution:** A new `knowledge_base` module that is a thin collection + scoping + agent-tool layer, reusing `file_rag`'s retrieval engine and the `file` module's ingest **unchanged**. A KB is a named, owner-scoped set of `file_id`s; retrieval resolves the KB to its file_ids and calls the existing `file_rag::retrieval::semantic_search(scope_ids, …)`. No new chunk/embedding tables, no new retrieval code.
**Basis:** codebase — `semantic_search` already takes an arbitrary `scope_ids: &[Uuid]` (retrieval.rs:85) and `files_mcp` already calls it that way; `citations` set the precedent of a thin module reusing `lit_search`'s engine.

### DEC-2: Data model — collections → documents → chunks → embeddings.
**Resolution:** `knowledge_bases` (owner-scoped) → `knowledge_base_documents` (M:N to `files`) → **chunks/embeddings reuse the existing `file_chunks` table** (produced by `file_rag` at upload). Attachment via `conversation_knowledge_bases` + `project_knowledge_bases`. No per-KB chunk partition.
**Basis:** codebase — mirrors `project_files`/`project_bibliography`; `file_chunks` already keys chunks by `file_id` + carries full provenance.

### DEC-3: Chunking strategy.
**Resolution:** Reuse `file_rag`'s existing page-aware char-window chunker (default 1200 chars / 200 overlap, admin-tunable) as-is for v1. Structure-aware/semantic chunking and Anthropic "contextual retrieval" enrichment are **out of scope for v1** (documented as future in CLAUDE.md).
**Basis:** codebase — chunking is already implemented and shared; changing it would re-index all `file_rag` consumers. Research flagged structural/contextual chunking as the top *future* upgrade, not a v1 blocker.

### DEC-4: Embedding model — reuse memory's? per-KB?
**Resolution:** All KBs share the **single deployment-wide `file_rag` embedding model** (`file_rag_admin_settings.embedding_model_id`), i.e. one shared embedding space. **No per-KB embedding model** in v1 (it would require per-KB chunk partitioning, since `file_chunks.embedding` is one fixed dimension). The KB module adds **no** settings singleton — it inherits file_rag's admin config (and the existing file_rag admin settings UI).
**Basis:** codebase — `file_chunks.embedding` is a single `halfvec(N)` column with one HNSW index; mixing embedding spaces in one index is impossible (confirmed by research §4). file_rag's model may or may not equal memory's — orthogonal.

### DEC-5: Retrieval — hybrid? rerank? **(confirm with user)**
**Resolution:** Reuse file_rag's hybrid (vector + FTS, RRF-fused) retrieval unchanged. **No reranker in v1.** A cross-encoder / LLM-rerank stage is documented as the next increment (research §3 shows it is the highest-value *next* add, but it needs a rerank model or extra LLM calls).
**Basis:** convention — ship the smallest correct thing on top of the proven engine; reranking is a bounded, additive follow-up. (Confirm the user is OK deferring rerank to v2.)

### DEC-6: How the agent USES it — inject before_llm_call, or an MCP tool? **(confirm with user)**
**Resolution:** **MCP tool** `search_knowledge`, auto-attached when ≥1 KB is bound to the conversation; **no `before_llm_call` chunk injection**. The chat extension only sets the attach flag + injects a one-line note naming the available KBs (data-not-instructions). This is deliberately the opposite of the current raw-prepend path.
**Basis:** convention + research §7 — Anthropic's "just-in-time context loading" and every recent built-in (web_search/lit_search/citations) expose retrieval as an on-demand tool; injecting 500 docs' worth of context on every turn is exactly the anti-pattern this feature removes.

### DEC-7: Citation / source-linking.
**Resolution:** `search_knowledge` returns each hit's `{file_id, file name, page_number, char_start, char_end, score}` in `structuredContent`. The chat UI renders citation chips that open `FilePreviewDrawer` at `page_number` via `Stores.File.requestPreviewPage`. Char-span highlight overlay is **out of scope for v1** (page-level deep-link only; the PDF viewer renders server-side page images, not selectable text).
**Basis:** codebase — `SemanticHit` already carries page + char span (retrieval.rs:36-45); the PDF viewer already deep-links to a page image (`file/viewers/pdf/body.tsx`). Char-highlight would need a text-layer viewer (not present).

### DEC-8: Scoping — per-user / per-project / shared? **(confirm with user)**
**Resolution:** **v1 = user-owned KBs** (owner-scoped, like memory/file_rag chunks which are `user_id`-scoped). A KB may be **attached** to the owner's conversations and projects. **Shared / org-wide / admin KBs are out of scope for v1** — they require chunks readable across users, a cross-cutting RBAC change to `file_chunks`' owner-scoping. Documented as the primary v2 extension.
**Basis:** codebase — `file_chunks.user_id` and `semantic_search`'s `user_id = $N` defense-in-depth filter make retrieval strictly owner-scoped today; honoring that keeps v1 correct and small. (Confirm the user doesn't need shared/lab-wide KBs in v1 — this is the most likely override for a "lab's protocols" use case.)

### DEC-9: Ingestion UX + scale limit.
**Resolution:** Two paths — (a) bulk multipart upload to a KB (drag-drop a folder), each file routed through `file::ingest::ingest_bytes` (which fires `file_rag`'s `spawn_index`), then attached; (b) attach already-uploaded `file_id`s. Per-document `index_status` (pending/indexed/failed) surfaced from `file_chunks` counts; the existing boot-time backfill self-heals stragglers. **Cap = `KB_MAX_DOCUMENTS = 2000`** (well above the 500-doc bar; HNSW handles incremental inserts), enforced atomically, 422 on overflow.
**Basis:** codebase — mirrors project bulk upload + `attach_file_capped` + the 422 cap idiom; research §8 confirms pgvector+HNSW is comfortable at 500 docs → ~hundreds of k chunks.

### DEC-10: Connected-vs-airgapped embedding story.
**Resolution:** Inherit file_rag's graceful degradation verbatim: with **no** embedding model configured, `search_knowledge` runs **FTS-only** (Postgres `tsvector`, keyword) — fully functional airgapped; with a model configured, it runs hybrid. No KB-specific enable/disable beyond file_rag's `enabled`/`semantic_enabled`.
**Basis:** codebase — `plan_arm(has_vector, fts_enabled)` already collapses to the FTS arm when no model is set (retrieval.rs); memory/file_rag both ship this.

### DEC-11: Chat-extension order + MCP built-in id + init order.
**Resolution:** Chat-extension `order = 24` (free, between control_mcp=22 and memory=25, before MCP=30). MCP id = `Uuid::new_v5(NAMESPACE_URL, b"knowledge_base.ziee.internal")`. Module `init` order = 104 (>65 so `mcp_servers` exists; after file_rag=87). Loopback route `/api/knowledge-base/mcp`.
**Basis:** codebase — order table verified (mcp.rs / extension.rs across modules); 24 and 104 are unused; string namespace is unique.

### DEC-12: Desktop.
**Resolution:** The module runs on the embedded desktop server (pgvector is present — memory/file_rag already run there); it is **NOT** added to `CORE_MODULE_BLOCKLIST`. Desktop needs only OpenAPI/TS regen + `npm run check`. No desktop-backend force-enable (KB inherits file_rag's default `enabled=TRUE`).
**Basis:** codebase — `loader.ts` blocklist holds only `user-profile`/`server-update`; memory/file_rag are not blocklisted.

### DEC-13: Permissions.
**Resolution:** `knowledge_base::use` (list/search/attach — granted to the Users group by migration 134) and `knowledge_base::manage` (create/edit/delete/upload). Administrators inherit both via the `*` wildcard. `search_knowledge` and `list_knowledge_bases` tools require `knowledge_base::use`; both are read-only ⇒ approval-bypassed.
**Basis:** codebase — mirrors web_search/citations `use`+`manage` split and the grant-to-Users migration idiom (104/98).

### DEC-14: Naming.
**Resolution:** Module `knowledge_base` (server) / `knowledge-base` (ui); route base `/knowledge`; MCP server display "Knowledge Base"; tool `search_knowledge`. No `ziee-chat` strings anywhere.
**Basis:** convention — snake_case server / kebab-case ui module naming; app-name rule `[[feedback_naming_ziee]]`.

### DEC-15: `document_count` maintenance.
**Resolution:** Denormalized `knowledge_bases.document_count`, updated inside the same transaction as attach/remove; a repository invariant test guards it. (Alternative — always `COUNT(*)` — rejected to keep list views cheap at 2000 docs/KB.)
**Basis:** convention — matches the denormalized-count pattern used elsewhere; correctness pinned by TEST-9.

Every decision above is resolved. Four are flagged **(confirm with user)** —
DEC-5 (defer rerank), DEC-6 (tool vs inject), DEC-8 (user-owned vs shared KBs),
and by extension DEC-4 (single shared embedding model) — and are repeated in the
halt message for override before implementation begins.
