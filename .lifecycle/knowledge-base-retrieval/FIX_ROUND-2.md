# FIX_ROUND-2 — iteration blind audit (FB-1..15 rebuild)

Phase 6 ran a fresh 3-agent BLIND audit (diff-only, no reasoning handed over) over
`git diff origin/main...HEAD` (source, generated files excluded), covering 11
angles: correctness · error-handling · concurrency · security · perms/authz ·
api-contract · state-management · patterns-conformance · a11y · perf ·
tests-quality. Every source hunk is covered by ≥3 angles (AUDIT_COVERAGE.tsv).

## Confirmed findings — all FIXED

- **F1 (MED-HIGH) correctness/state** — `retryAllFailed` retried only the loaded
  page but the button label used the KB-wide `indexing_summary.failed` count →
  misleading + silent no-op for failures on other pages. FIXED: the button now
  counts the CURRENT page's retryable rows (`retryablePageCount`) and reads
  "Retry N failed on this page"; it only shows when the loaded page has retryable
  docs. `KnowledgeBaseDocumentsPanel.tsx`.
- **F2 (MED) correctness** — `citationTokenize` rewrote `[n]` inside code
  spans/fences AND numeric array indices (`arr[1]`), corrupting displayed code /
  creating false chips. FIXED: (a) split on fenced/inline code and never rewrite
  inside it; (b) tightened the regex to `(?<![\w\]])\[(\d{1,3})\](?![(:])` — not
  preceded by a word char or `]`, not followed by `(` or `:` — so `arr[1]`,
  `[Smith][1]`, `[1]: url` are all left alone. New unit assertions cover each.
- **F3 (MED) a11y** — the "Used in" project/chat jump `Tag`s were mouse-only.
  FIXED: added `role="button"` + `tabIndex` + `aria-label` + Enter/Space
  `onKeyDown`. `KnowledgeBaseDetailPage.tsx`.
- **F4 (MED) correctness** — `CitationChip` resolved `[n]` to the FIRST
  transparency card; `[n]` means the MOST RECENT search. FIXED: `querySelectorAll`
  → take the LAST card. `CitationChip.tsx`.
- **F5 (LOW-MED) state/race** — `loadInherited` had no latest-wins guard (fast
  project→project-less nav could leave stale inherited chips). FIXED: capture the
  conversation id at call, discard the late resolve if it changed.
- **F6 (LOW) perf** — `loadUsage` refetched on every `sync:file_index_state`
  (fires per-doc during bulk indexing). FIXED: usage now refreshes on
  `sync:knowledge_base` (attach/detach) instead, not the per-document stream.
- **F7 (LOW) concurrency** — geometry-backfill single-flight flag could wedge
  `true` on a panic in `run_inner`. FIXED: reset via a `Drop` guard.
- **F8 (LOW) error-handling** — office `extract_geometry` leaked the temp input
  file if `create_dir_all` failed. FIXED: cleanup on that error path.
- **F9 (LOW) patterns** — a stray `import` after a const in
  `KnowledgeBaseDetailPage`. FIXED: moved up with the other imports.

## Triaged — accepted (documented), not defects introduced by this diff

- `searchable` over-counts `no_text` docs in the indexing-incomplete banner — a
  pre-existing calc MIRRORED from the shipped `search_knowledge` MCP handler;
  cosmetic; kept consistent with the existing path rather than diverging.
- `search_kb` query length unbounded — auth-gated + parameterized; at most a
  self-inflicted CPU cost by the authenticated owner.
- `refreshLoadedDocuments` steps back exactly one page on an emptied page — bounded
  because bulk-remove selection is per loaded page.
- `fileFindQuery` doesn't refire on an identical string / isn't cleared on find
  close — clicking the SAME passage twice is a rare no-op; the common case works.

## Re-review

All MED+ findings fixed; ui `tsc` clean; backend `cargo check` clean; tokenizer
unit assertions extended to lock F2. The accepted LOWs are documented rationale,
not silent dismissals.

**New confirmed findings:** 0
