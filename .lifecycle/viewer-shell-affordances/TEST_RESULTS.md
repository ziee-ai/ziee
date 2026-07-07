# TEST_RESULTS — viewer-shell-affordances

Frontend-only diff (`src-app/ui/**`). Static gate + unit + e2e (real backend for the
interactive viewer flows; gallery/backend-free for the shell-chrome render), per the
tiers enumerated in TESTS.md.

## Frontend gate

- **npm run check (ui): PASS** — tsc + biome guardrails + lint:colors +
  lint:settings-field + lint:icon-action + check:kit-manifest + check:testid-registry
  + check:design-spec + check:gallery-coverage + check:state-matrix + check:overlay-registry
  all green.

(Only the `ui` workspace is touched — no `src-app/desktop/ui/**` source in the diff.
The shared file module is consumed by desktop via vite fallback; desktop `tsc --noEmit`
was also verified green.)

## UI Build Gate (runtime-health)

- The one touched gallery surface, `overlay-file-preview-drawer`, reports **0 HIGH**
  runtime findings (0 console-error / page-error / request-failed / AA-contrast), only
  2 pre-existing LOW spacing nits. The global `gate:ui` exit is red on **pre-existing,
  unrelated** environmental noise (backend-refused requests + katex fonts + injected-500
  error-state surfaces across the whole gallery) — none on a surface this feature touches.
- A `useNavigate()`-outside-Router crash in the file-preview overlay (surfaced by the
  gallery) was found and fixed (FullPageButton now degrades to an anchor via
  `useInRouterContext`).

## Unit (node --test) — 19 assertions, all pass

- **TEST-1**: PASS — `image/zoom.test.ts` clamp/step math.
- **TEST-2**: PASS — `image/zoom.test.ts` `clampTranslate` pan bounds.
- **TEST-3**: PASS — `find/matcher.test.ts` case-insensitive non-overlapping matches.
- **TEST-4**: PASS — `find/highlightSupported.test.ts` feature-detect (false off-DOM + stubbed true branch).
- **TEST-12**: PASS — `find/offset.test.ts` cross-segment offset mapping.

## E2E — Playwright

Real backend (`tests/e2e/file/*`, `--workers=1`): the full run reported 17 passed / 3
failed / 2 skipped; the 3 failures were 1 unrelated `file-rag` real-LLM test (skipped-key
env) + 2 of these specs whose ASSERTIONS were wrong (not the product) — both fixed and
re-run green (2 passed, 55s). The 2 skipped are `file-rag` real-LLM (placeholder keys).

- **TEST-5**: PASS — `file/image-zoom.spec.ts` (zoom → actual, scale>1, drag-pan transform change, keyboard arrow-pan, Fit reset).
- **TEST-6**: PASS — `file/find-in-document.spec.ts` (find button + Ctrl-F open, "n / m" count, next/prev wrap, No results, Esc close).
- **TEST-7**: PASS — `file/word-wrap.spec.ts` (long line overflows → wrap removes horizontal overflow → toggle back).
- **TEST-8**: PASS — `file/copy-selection.spec.ts` (selection copied; empty selection warns + clipboard untouched).
- **TEST-9**: PASS — `file/open-in-new-tab.spec.ts` (window.open called with the `/download-with-token?token=` URL).
- **TEST-10**: PASS — `file/full-page-view.spec.ts` (navigate to /files/:id, drawer closes, back returns; bogus id → not-found).
- **TEST-11**: PASS — `visual/viewer-affordances.spec.ts` (backend-free gallery: open-tab + full-page shell buttons render with accessible names, light + dark).
