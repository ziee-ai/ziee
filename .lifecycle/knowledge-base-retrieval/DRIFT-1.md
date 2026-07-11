# Drift Round 1 — knowledge-base-retrieval (implementation vs PLAN)

Reconciliation of the implemented feature against `PLAN.md` after phases R/I/K/C
+ frontend were built. Each divergence is classified and resolved.

- **DRIFT-1.1** — verdict: impl-wins — PLAN did not foresee that the chat composer
  needs to READ a conversation's attached KBs to hydrate its chip state on load;
  only attach/detach (PUT/DELETE) existed. Added `GET /conversations/{cid}/knowledge-bases`
  and `GET /projects/{pid}/knowledge-bases` (owner-scoped, enriched) + repo
  `attached_kbs_for_conversation`/`attached_kbs_for_project`. Plan intent (a
  composer that shows current grounding) is served; the read endpoints are the
  minimal addition.

- **DRIFT-1.2** — verdict: impl-wins — ITEM-35/36 envisioned inline `[n]` citation
  chips in the assistant prose. The `search_knowledge` tool design cites by
  file/page in natural language (no `[n]` index is emitted), so there is nothing
  for a chip to key off. Dropped the `[n]` chips; the retrieval-transparency card
  (per-passage source·page·score with "Open source") + the exact-passage
  highlight overlay deliver the same jump-to-cited-passage intent. Recorded in the
  Part C-UI polish commit.

- **DRIFT-1.3** — verdict: impl-wins — a desktop-ui mirror of the KB module was
  assumed, but `desktop/ui` is a thin management shell that omits the ENTIRE
  chat/knowledge surface (no chat/file/projects/citations/literature/file-rag
  modules). Mirroring KB would diverge from that established pattern. The desktop
  binary embeds the server and exposes the KB/File REST surface, so the only
  desktop change needed was an api-client regen (done); no desktop UI module.

- **DRIFT-1.4** — verdict: resolved — the exact-passage highlight (ITEM-37) was
  planned as a PDF overlay consuming ingest-time geometry. Implemented as a
  `PdfHighlight` shared store + `PdfController.setHighlights` overlaying
  percent-positioned rects inside pdf.js `.page` divs (auto-tracks zoom,
  re-injected on `pagerendered`). Matches the plan; the store-based coordination
  follows the file module's `types/viewer.ts` convention (no threaded props).

- **DRIFT-1.5** — verdict: none — the reranker delivery (Parts R/H) landed as
  planned: a `rerank` model capability threaded provider→dispatch→local-runtime→
  file_rag retrieval, delivered via the ziee-ai/hub BGE-reranker model, gated OFF
  by default. No divergence.

**Unresolved drifts:** 0
