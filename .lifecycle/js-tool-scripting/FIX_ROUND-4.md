# FIX_ROUND-4 — js-tool-scripting (verification round)

A fresh blind verification pass over the four FIX_ROUND-3 fixes (diff-only context,
no reasoning handed over), across {tests-quality, correctness}:

- **TEST-15 / `context_tool_use_names`** — verified the `PaginatedMessages` →
  `contents` → `content_type=="tool_use"` → `content.name` read is correct, and the
  negative assertion cannot false-pass (the positive `any(contains "run_js")` runs
  first, so an empty/misrouted response fails loudly before the `!contains
  "get_tool_result"` check).
- **run-js-real-llm** — the `[data-testid^="mcp-toolcall-status-"][data-status="completed"]`
  selector matches only a successful (non-error) run_js and is `sr-only` (a 1px box,
  so Playwright `toBeVisible()` passes); `getByText('42')` is valid.
- **run-js-tool-scripting** — `SOURCE_TONE.script='info'` → the kit Tag's
  `outlineTones.info` contains `text-info`, which `toHaveClass(/text-info/)` matches
  and distinguishes from the `default` fallback (`text-foreground/80`); the literal is
  in source so it survives Tailwind purge.
- **comment fix** — matches the stub script (`6*7`).

All fixes are correct and introduce nothing new.

**New confirmed findings:** 0
