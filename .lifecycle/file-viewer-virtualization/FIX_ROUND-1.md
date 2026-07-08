# FIX_ROUND-1 — file-viewer-virtualization

Merged the phase-6 LEDGER (18 rows, 12 angles), fixed every CONFIRMED finding,
then re-ran a full blind round (2 fresh diff-only agents: effects/concurrency +
tests/a11y).

## Confirmed phase-6 findings fixed

- **F1 (concurrency, HIGH)** — RawCodeView: no per-request cancel guard. Added a
  monotonic `genRef` bumped on every (text/lang/theme) reset; each highlight
  captures `gen` and discards its resolution when `genRef.current !== gen`, so a
  prior file's/theme's HTML can never land in the new state. (LEDGER lines 175/181)
- **F2 (state/correctness, HIGH)** — observer never (re)attached after the
  OverlayScrollbars `defer` null-viewport first run, and never re-fired after a
  theme flip. Reworked into an `attach()` that rAF-retries until the viewport
  exists, keyed on a new `readyGen` state (bumped when shiki resolves) so a theme
  reset re-observes → the IO's initial callback re-highlights visible chunks.
  (LEDGER lines 239/240)
- **F3 (perf, MEDIUM)** — a single chunk highlight re-rendered all N chunk vdoms.
  Extracted a `memo`'d `CodeChunk` (primitive props html/reservedPx/index); the
  N-1 unchanged chunks reuse their stable `plainHtml` string ref → memo skips
  them. (LEDGER line 176)
- **F4 (a11y, MEDIUM)** — per-chunk `<pre tabindex=0>` created one tab-stop per
  chunk. Removed it from the plain builder AND stripped shiki's auto-added
  tabindex via a transformer `pre` hook. (LEDGER line 88)
- **F5 (tests-quality, MEDIUM×2)** — the parse-cap tests re-implemented the
  `n > CONST` predicate instead of exercising production code, and the XLSX
  truncation lived inline in XlsxBody (untested). Extracted a shared
  `capRows(rows, cap)` into parse.ts used by BOTH `parseDelimitedText`
  (now `cap`-injectable) and XlsxBody; rewrote the tests to drive the REAL
  truncated:true slice branch. (LEDGER lines 30/62)
- **F6 (tests-quality, MEDIUM)** — the main-config large-text e2e asserted Shiki
  colored spans, which flake on the BUILT server (Shiki-under-preview known
  issue). Made it highlight-independent (chunk-slot count, last-line text present
  + visible after scroll, find, word-wrap); the highlight-on-window assertion now
  lives ONLY in the gallery visual spec, which runs in dev where Shiki applies.
  (LEDGER line 49)
- **F7 (patterns, LOW)** — softened the "Mirrors pdf/body.tsx exactly" comment
  (rootMargin/eager-first differ). (LEDGER line 238)

Rejected (with rationale, in the ledger): plain-HTML O(N) build is the intended
DEC-3 find floor; find MutationObserver re-walk is inherent + debounce-mitigated;
wrap reserved-height estimate is corrected by `contain-intrinsic-size:auto`;
escapeHtml is complete for the text-only injection context (no XSS); highlight-
swap unit test is a supporting property test (the e2e find is the real-path
proof); DelimitedTable viewRef staleness is pre-existing, not introduced.

Post-fix: tsc clean (ui + desktop), all 20 unit tests green, biome + color +
guardrail lints clean.

## Re-audit round result

- Agent A (concurrency/correctness/state/perf): 0 confirmed — verified the gen
  guard closes the stale-write (React flushes the reset effect's cleanup + the
  observer effect synchronously with no task interleave; theme-flip keeps the
  same chunk elements so a late old-IO fire uses the new theme + correct index),
  the rAF/IO cleanup is leak-free, the requested-Set lifecycle is sound, and the
  CodeChunk memo skips the N-1 unchanged chunks.
- Agent B (tests-quality/a11y/correctness/patterns): verified the parse-cap tests
  now drive real `parseDelimitedText`/`capRows`, the XLSX cap is genuinely covered
  via the shared predicate, the e2e specs are Shiki-independent with correct
  readout formats/testids, and the xlsx build/upload path is correct — BUT
  surfaced ONE new finding.

### New finding this round (fixed in round 2)

- **a11y (MEDIUM)** — F4 removed per-chunk tabindex to kill the many-tab-stops
  anti-pattern, but removed the ONLY keyboard-focusable element in the scroll
  region, so keyboard-only users can't arrow-scroll the code on browsers without
  auto-focusable scrollers. The correct fix is ONE focusable scroll container,
  not zero → addressed in FIX_ROUND-2.

**New confirmed findings:** 1
