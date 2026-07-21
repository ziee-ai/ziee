# DRIFT-1 — render-equations

Implementation audited against PLAN.md / TESTS.md after all items were written.

- **DRIFT-1.1** — verdict: impl-wins — PLAN.md's *Files to touch* listed only
  `tests/e2e/chat/markdown-rendering.spec.ts`, but TESTS.md's TEST-10 and TEST-11
  target `tests/e2e/skills/skill-detail-drawer.spec.ts` and
  `tests/e2e/workflows/run-step-expanders.spec.ts`. The plan's file list was simply
  incomplete — the tests were correctly enumerated in phase 3, and hosting them in
  the existing specs is right (each already owns the seeding machinery the test
  needs: a route-mockable SKILL.md body fetch, and the dev-only `mock:` step
  short-circuit). PLAN.md amended to list both; phases 1–3 re-run.

- **DRIFT-1.2** — verdict: resolved — the plan called for the byte-identical proof
  to live as "literal expected strings" in `markdownPreprocess.test.ts`. That is what
  shipped, but it was *derived* from a stronger one-off harness: the pre-change
  implementation was extracted with `git show HEAD:…` and run side-by-side with the
  reworked one over a 26-input non-math corpus (reference links full/collapsed/
  shortcut, unresolved refs, footnotes, array indices, external/local/`data:`/nested
  images, fenced + `~~~` + inline code, tables, blockquotes, lists, headings, empty
  and trailing-newline inputs). All 26 were byte-identical. The harness was scratch
  and is not committed; the committed test pins the representative subset with
  literal expectations, exactly as planned. No divergence in shipped code.

- **DRIFT-1.3** — verdict: none — ITEM-8 called for fixing "the two comment sites"
  citing `[[no-katex-remark-rehype]]`. Both were fixed: the file-header docblock (now
  describing the wired math pipeline and the `\[…\]` normalization) and the mermaid
  test's inline rationale (which additionally claimed the project installs no markdown
  plugin packages — false since `@streamdown/code`/`math` shipped; corrected to point
  at the custom `renderers` entry that actually owns mermaid). `grep` confirms no
  live reference to the retired directive remains outside the header note recording
  its retirement.

- **DRIFT-1.4** — verdict: none — plan said `STREAMDOWN_PLUGINS` and
  `chatMarkdownPlugins` stay untouched. Confirmed: neither file is in the diff. The
  audit note that the skill/workflow surfaces pass `STREAMDOWN_PLUGINS` (not
  `chatMarkdownPlugins`) was respected — only the children expression changed at
  those call sites.

- **DRIFT-1.5** — verdict: none — plan predicted the two chat renderers would be in
  lockstep after ITEM-6. Confirmed: both now read
  `preprocessMarkdown(citationTokenize(text))`, with the extensions variant keeping
  its `isUser ?` branch (user messages skip citation tokenizing there; the older
  renderer short-circuits user messages to plain text before reaching Streamdown at
  all, so the two remain behaviorally equivalent for the assistant path).

**Unresolved drifts:** 0
