# PLAN_AUDIT — render-equations

Audit of PLAN.md against the actual codebase, before any code is written.

## Breakage risk

**The single high-risk item is ITEM-5** — `preprocessMarkdown` is the shared
preprocessor for *all* markdown rendering. Today it is called by 2 of the 5
Streamdown call sites; after ITEM-6/7 it will be called by all 5. Any behavior
change on non-math input is therefore a fleet-wide regression, not a local one.
Mitigations, all mandatory:

- The nested math-span split (ITEM-5) must be **purely additive** for input with no
  math: with zero `$` spans, `s.split(/(\$\$…|\$…\$)/)` yields a one-element array,
  the `j += 2` loop runs once over the whole string, and `sub.join('')` reproduces
  it byte-for-byte. Non-math input must be provably identical — asserted directly.
- The widened early return must not *narrow* the existing one. Current guard is
  `md.indexOf('[') === -1` → return. New guard adds `&& md.indexOf('\\(') === -1`,
  which only lets **more** strings through to processing. A string with `[` still
  processes exactly as before.
- `normalizeMathDelimiters` runs FIRST inside each code-split segment, so the
  reference-link and image passes see its output. Since it only ever emits `$`,
  `\n` and the original inner text, it cannot manufacture a `[`…`]` pair that the
  reference pass would then rewrite — but the nested math-span split makes that
  independent of the argument.

Second-order risks reviewed:

- **`isSameOriginImage` touches `window.location.origin`.** In the `node:test`
  runner there is no `window`; the `new URL(...)` call would throw
  `ReferenceError`, which the existing `try/catch` swallows → returns `false`
  (treated as external → placeholder). So a unit test that includes an image is
  *deterministic but not browser-faithful*. `markdownPreprocess.test.ts` must
  therefore assert **math + code protection**, and leave image behavior to the
  existing e2e/browser path. Recorded, not worked around.
- **ITEM-6 changes what reaches Streamdown for every assistant chat message**
  (`chat/components/TextContent.tsx` gains reference-link inlining + image
  blocking). This is the same code the sibling `extensions/text` renderer has run
  in production, so the risk is low and the divergence is itself the bug being
  fixed — but it is a real behavior delta and must be exercised by the existing
  full `markdown-rendering.spec.ts` suite, not just the new math cases.
- **ITEM-7's two surfaces render trusted-ish content** (`SkillDetailDrawer` shows
  a skill's `description` + on-disk `SKILL.md` body; `StepOutputExpander` shows
  workflow step output, already short-circuited to `<pre>` when `isJson`). Blocked
  external images and inlined reference links are both improvements there. No
  security regression: the change only ever *reduces* what loads (external images
  become placeholders).
- **No streaming regression path.** `normalizeMathDelimiters` is a pure function of
  the current buffer; an unclosed `\[` simply fails to match and passes through, so
  the partial render is byte-identical to today's.

## Pattern conformance

- **Reference module for the new pure helper**: `modules/chat/core/utils/citationTokenize.ts`
  (+ its colocated `citationTokenize.test.ts`). Verified shape: module-scope
  `const …_RE = /…/g` with an explanatory comment above each regex, one exported
  pure function, a second exported helper where needed, JSDoc on the exported
  function. `normalizeMathDelimiters.ts` mirrors this exactly.
- **Test runner**: `node:test` + `node:assert/strict`, colocated `*.test.ts`,
  source imported with an explicit `.ts` extension, no `describe` wrapper —
  confirmed against `citationTokenize.test.ts`, `collapsible.test.ts`,
  `conversationDisplayLabel.test.ts`. `vitest.config.ts` is scoped to
  `src/**/*.store.test.ts` and is NOT the right runner here.
- **Code-protection idiom**: `split(<code regex>)` → transform even indices →
  `join('')`. Confirmed in both `citationTokenize.ts` and `markdownPreprocess.ts`.
  ITEM-5 extends `markdownPreprocess.ts`'s existing loop rather than adding a
  parallel one.
- **Lookbehind precedent**: `citationTokenize.ts:19` already ships `(?<![\w\]])`,
  so ITEM-1's `(?<!\\)` raises no browser-support question.
- **Module placement**: `src/components/common/` is correct — it already holds
  `markdownPreprocess.ts` and `streamdownPlugins.ts`, i.e. the cross-module
  markdown layer. Putting the normalizer under `modules/chat/` would be wrong,
  since 3 of its 5 consumers are outside chat.
- **Deviation from plan-time assumption, corrected**: `SkillDetailDrawer` and
  `StepOutputExpander` pass `STREAMDOWN_PLUGINS`, not `chatMarkdownPlugins` —
  confirmed by reading both. That is irrelevant to this fix (the normalizer is
  upstream of the plugin config) but it means ITEM-7 must not accidentally
  "harmonize" the plugin prop; only the children expression changes.

## Migration collisions

**None.** No migration is added. The server no longer carries
`src-app/server/migrations`; migrations live per-crate under the `sdk` submodule
(`sdk/crates/ziee-{auth,file,notification,onboarding,seed}/migrations`) and
`src-app/desktop/tauri/migrations`. This feature touches none of them, so there is
no migration-number surface to collide on. See BASE.md.

## OpenAPI regen

**Not required.** No Rust type, route, request or response shape changes. Neither
`openapi.json` nor `api-client/types.ts` is touched in `src-app/ui` or
`src-app/desktop/ui`; `just openapi-regen` is a no-op for this branch and the C3
regen-parity merge gate is vacuous. Correspondingly, R2-5 (e2e `/api/` route-mocks
must match a live route) applies only to mocks already present in
`markdown-rendering.spec.ts` — this change adds no new route mock, it reuses the
existing `seedAssistantWithText` helper.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — mirrors `citationTokenize.ts` structure; the combined-alternation regex with a single `lastIndex` walk is what prevents a `\(` inside an already-consumed `\[…\]` from re-matching; lookbehind precedent exists in-tree.
- **ITEM-2** — verdict: PASS — inline needs no block positioning, so it is a pure `$inner$` substitution; `singleDollarTextMath: true` is already enabled in `streamdownPlugins.ts`, which is precisely what makes `$…$` renderable.
- **ITEM-3** — verdict: CONCERN — the continuation-prefix reconstruction is the most intricate logic in the change and has documented imperfect cases (lazy-continuation blockquotes, setext headings). Not blocking: each imperfection degrades to *unconverted or slightly-misplaced math*, never to corrupted surrounding markdown, and the list/blockquote/paragraph cases that actually occur are covered by enumerated tests. Requires the widest test matrix of any item.
- **ITEM-4** — verdict: PASS — every guard is a "return the match unchanged" early-out, i.e. it can only ever preserve today's behavior. Guard (A) is what bounds an unclosed-delimiter runaway during streaming.
- **ITEM-5** — verdict: CONCERN — highest blast radius in the change (shared by all 5 render sites after ITEM-6/7). Not blocking, but gated on the byte-identical-on-non-math-input proof and the full existing markdown test surface staying green, both enumerated in TESTS.md. The `$$ a[1] $$` + `[1]: url` case is the permanent regression guard.
- **ITEM-6** — verdict: PASS — brings the older chat renderer into lockstep with its sibling, which is the pre-existing correct behavior. One-line change; the divergence it removes is itself a latent bug.
- **ITEM-7** — verdict: CONCERN — deliberately broader than issue #177. Not blocking given the two recorded obligations (verify both added behaviors on these surfaces by rendering, per B7; disclose in a "Beyond #177" PR section) and the documented fallback to a math-only entry point if verification surfaces a problem. Confirmed by reading both files that only the children expression changes; the `STREAMDOWN_PLUGINS` prop stays as-is.
- **ITEM-8** — verdict: PASS — corrective, not additive. `tests/e2e/chat/markdown-rendering.spec.ts:227` asserts `.katex` count is 0 while `streamdownPlugins.ts` wires `createMathPlugin` and `index.css:9` imports the KaTeX stylesheet; the test contradicts shipped code and would fail independently of this feature. DEC-5 records the formal retirement of the `[[no-katex-remark-rehype]]` directive.
- **ITEM-9** — verdict: PASS — descoped with a recorded rationale (DEC-6) and an approved `DESCOPED:` disposition in DECISIONS.md. The reasoning is concrete, not deferral-by-fatigue: the common emission is already `$$\begin{align}…$$` (handled today), and a naive `\begin{…}` pattern would double-wrap and destroy it, so correct support first requires teaching the splitter about existing `$$` spans.

**No BLOCKED verdicts.** Two CONCERNs (ITEM-3, ITEM-5) and one scope CONCERN
(ITEM-7), each with an explicit mitigation carried into TESTS.md.
