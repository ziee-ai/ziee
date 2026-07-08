# message-scroll-perf — TEST_RESULTS

Frontend-only diff (`src-app/ui/**`), so the phase-8 gates are the `ui` static
gate + the enumerated `tier: e2e` specs. Backend was NOT touched (no integration
tier).

## Frontend static gate

`npm run check (ui): PASS`

(tsc + biome guardrails + lint:colors + lint:settings-field + lint:adjacent-inline
+ lint:icon-action + lint:logical-direction + lint:tooltip-placement +
check:kit-manifest + check:testid-registry + check:design-spec +
check:gallery-coverage + check:gallery-crawl + gallery:check-fixtures +
check:state-matrix + check:overlay-registry — all green, exit 0.)

## Unit (node:test) — all 38 assertions green

- **TEST-1**: PASS  (`estimateMessageHeight.test.ts` — content-aware estimate: heavier for table/image/code/tool blocks, monotonic in text length, capped, null-safe, width-sensitive)
- **TEST-2**: PASS  (`measuredHeightCache.test.ts` — width-bucketed store/read, stale-width miss, sub-bucket hit, folds itemSizeCache, builds seed only for cached ids)
- **TEST-3**: PASS  (`reservedImageBox.test.ts` — aspect-ratio when dims present, min-height reserve when absent, released on load, non-positive fallback)
- **TEST-4**: PASS  (`imageSrcPolicy.test.ts` — blocks external / `data:` / protocol-relative `//host` / backslash `/\host` / opaque / malformed; allows same-origin)
- **TEST-5**: PASS  (`scrollAnchor.utils.test.ts` — `anchorRestoreNeeded` idempotency guard + `indexRestoreOffset` clamp)

## E2E (Playwright, chromium) — all green

- **TEST-6**: PASS  (`message-scroll-perf.spec.ts` — geometry stability: initial≈final scrollHeight ratio in (0.65,1.5), mounted count bounded — the estimate fix)
- **TEST-7**: PASS  (`message-scroll-perf.spec.ts` — warm-start: remount seeds measured heights, warmSH/finalSH in (0.95,1.05))
- **TEST-8**: PASS  (`message-scroll-perf.spec.ts` — code-heavy conversation scrolls with zero app console/page errors; Shiki `<pre>` rendered; newest row virtualized out after scroll-up)
- **TEST-9**: PASS  (`message-scroll-image-stability.spec.ts` — inline image row reserves height before load; bounded post-load shift)
- **TEST-10**: PASS  (`lazy-load-messages.spec.ts` — prepend no-teleport anchor invariant `|scrollGrew − addedAbove| < 80`, with overscan 8)
- **TEST-11**: PASS  (`message-scroll-perf.spec.ts` — 100-row markdown table renders inside a height-capped message row)

## Regression guards (pre-existing specs, kept green)

- **virtualize-messages.spec.ts**: PASS (virtualization reduces mounted rows; overscan 8)
- **conversation-find.spec.ts**: PASS (find surfaces + jumps to a virtualized-out match)
- **lazy-load-jump-to-message.spec.ts**: PASS (deep-link centers + highlights an unloaded message)
- **inline-csv-height-stability.spec.ts**: PASS (inline CSV height stable — ITEM-4 sibling)

## Notes

- **gate:ui (runtime-health + visual)**: FAILS only on PRE-EXISTING / environmental
  issues, proven by running the identical gate on the merge-base (a101c851, no
  change): the baseline fails WORSE (172 surfaces with HIGH findings + the same
  visual failure, vs 51 on this branch). The chat-surface HIGH findings are KaTeX
  `@fs` 403s (Vite's fs-allow doesn't cover the worktree's symlinked node_modules)
  + a pre-existing `[FileStore] response.text is not a function` cassette bug; the
  one visual failure is `gallery-section-mermaid-block` (async mermaid overflow) —
  a component the diff never touches. ZERO runtime findings reference the changed
  code. So the touched surfaces carry no NEW runtime/visual regression.
- **E2E infra**: this shared box has a concurrent Claude session running e2e in
  another worktree; per-test Postgres contention caused many false ECONNREFUSED /
  pool-timeout failures. Every enumerated spec above was confirmed green in a
  contention-free window (`--workers=1`), reaching and passing its real assertions
  (0 ECONNREFUSED / 0 DB-create failures on the recorded runs).
