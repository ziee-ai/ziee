# DECISIONS — render-equations

Every human/product input the implementation needs, resolved up front.

### DEC-1: At which layer is the delimiter transform applied — raw string before Streamdown, or a remark plugin on the mdast?
**Resolution:** Raw-string pre-transform, composed into the existing `preprocessMarkdown`.
**Basis:** codebase — `remark-math` is a *micromark syntax extension* (`micromark-extension-math/dev/lib/syntax.js` registers on `codes.dollarSign` at tokenize time). A tree-level remark plugin runs after parsing, when LaTeX has already been mangled (`x_1 … y_1` consumed as emphasis, backslash escapes eaten), so it could never reconstruct the source. Streamdown also orders `remarkPlugins` before `plugins.math.remarkPlugin`, which does not help since micromark extensions all register pre-parse. The raw-string layer is both the only correct one and the established repo pattern (`citationTokenize`, `preprocessMarkdown`).

### DEC-2: Should bare `[ … ]` (no backslash) also be converted to math?
**Resolution:** No. Only `\[ … \]` and `\( … \)` are converted.
**Basis:** user/issue — #177's screenshot shows `[ \frac{d^2C(x)}{dx^2} … ]`, i.e. the backslash already consumed by markdown's character escape, proving the source was `\[`. Bare square brackets are ordinary prose and markdown link syntax; converting them would corrupt normal text at enormous scale for no gain.

### DEC-3: What happens when the LaTeX content itself contains a `$`?
**Resolution:** Inline (`\(…\)`) — skip the conversion, leave the text unchanged. Display (`\[…\]`) — convert normally, guarded only against a body line that is exactly `$$`.
**Basis:** codebase — for inline, `$a $ b$` would close the span at the inner `$` and corrupt the rest of the line; degrading to today's raw-text behavior is strictly better than corrupting the document. For display, the flow closer must be a line containing only `$$`, so an inner `$` (e.g. `\text{cost} = \$5`) is safe. Rejected alternative: escaping inner `$` to `\$` — micromark does not honor a preceding backslash inside the span, so it would not actually protect the delimiter.

### DEC-4: What happens to a `\[ … \]` inside a markdown table row?
**Resolution:** Downgrade to inline `$…$` rather than injecting a newline.
**Basis:** convention — a table row is newline-terminated, so injecting `\n` to reach block position would destroy the row. Inline math still renders; a slightly-less-prominent equation beats a broken table. Detected heuristically by the presence of `|` on the match's line, which over-triggers on a prose line containing a pipe — accepted, since the fallback is still working inline math.

### DEC-5: The repo carries a `[[no-katex-remark-rehype]]` directive and an e2e test asserting math does NOT render. Keep or retire?
**Resolution:** Retire it. Invert the test and remove the directive from its two comment sites.
**Basis:** codebase — the directive is already dead. `streamdownPlugins.ts` wires `math: createMathPlugin({ singleDollarTextMath: true })` and `index.css:9` imports `katex/dist/katex.min.css`, so math IS wired on this branch. `tests/e2e/chat/markdown-rendering.spec.ts:227` asserts `.katex` count is 0 and therefore contradicts shipped code; it would fail independently of this feature. Retiring it is corrective, not a policy reversal.

### DEC-6: Support bare `\begin{equation}…\end{equation}` / `\begin{align}…\end{align}` now, or defer?
**Resolution:** Defer. Descoped as ITEM-9 this round.
**Basis:** user — presented as an explicit option picker and the human chose defer. Concrete reasoning, not fatigue: KaTeX 0.16.47 supports these environments, so it is purely a delimiter question — but the far more common model emission is `$$\begin{align}…\end{align}$$`, which **already works today**. A naive `\begin{…}` pattern has no awareness of existing `$$` spans and would double-wrap that common case into `$$\n$$\begin{align}…`, destroying working output. Correct support first requires teaching the splitter about existing math spans; doing it half-way is a net regression.
- DESCOPED: ITEM-9 — bare `\begin{env}` support requires `$$`-span awareness in the splitter first; the common `$$\begin{align}…$$` emission already renders, and a naive pattern would regress it [approved: khoi 2026-07-21, via AskUserQuestion option picker "LaTeX envs → Defer, document it"]
- DESCOPED: ITEM-2 — inline `\( … \)` conversion cut entirely; the delimiters are POSIX BRE grouping and converting them corrupts sed/grep and LaTeX-escaping prose with no heuristic able to separate the cases. Inline math still renders via `$…$`. See DEC-12 [approved: khoi 2026-07-21, "INLINE HANDLING = DISPLAY-ONLY"]
- DESCOPED: ITEM-7 — skill drawer + workflow step output reverted entirely; they are the syntax-documenting genre and the audit proved the shared preprocessor rewrites their documentation examples. #177 is chat-only. See DEC-7 [approved: khoi 2026-07-21, "SKILL/WORKFLOW = REVERT ENTIRELY"]

### DEC-7: Do the skill drawer and workflow step output get the FULL `preprocessMarkdown`, a math-only entry point, or nothing?
**Resolution:** **REVERTED — neither. Both surfaces are OUT OF SCOPE.** `SkillDetailDrawer.tsx` and `StepOutputExpander.tsx` are restored to their `origin/khoi` state and are untouched by this PR, along with the two e2e specs that covered them. The fix is scoped to chat (`TextContent.tsx`) plus the file viewer, which already runs through `preprocessMarkdown`.
**Basis:** user — this reverses an earlier answer, on evidence the blind audit produced. The pre-registered fallback in the original DEC-7 ("if verification surfaces a problem, scope those two files down") fired: routing SKILL.md through the full `preprocessMarkdown` rewrites *documentation examples* — verified, a skill doc explaining markdown's shortcut-reference form has its own `[id]` example turned into a live link. SKILL.md bodies and workflow step output are precisely the genre that documents syntax, so they are the worst possible input for a syntax-rewriting pass. Issue #177 is a chat report; the broadening bought nothing for it. The human chose "revert entirely" over "math-only" so no second entry point is created either. The PR body states the exclusion and why.

### DEC-12: Is inline `\( … \)` converted?
**Resolution:** **No.** Only display `\[ … \]` is converted. Inline `\( … \)` passes through byte-for-byte unchanged, exactly as today.
**Basis:** user — presented as an option picker with verified evidence and the human chose display-only. `\(` and `\)` are byte-identical to POSIX BRE group syntax, so converting them corrupts ordinary prose: `sed -e 's/\(foo\)/bar/'` became `sed -e 's/$foo$/bar/'` (backslashes deleted, rendered italic), `Pattern \(a\|b\) matched` became `Pattern $a\|b$ matched`, and `To escape use \( and \) in LaTeX.` became `To escape use $and$ in LaTeX.` No heuristic resolves this: whitespace padding rejects the sed family but NOT the documentation sentence, which is padded exactly like real math. Since inline math already renders via `$…$` (`singleDollarTextMath` is enabled) and issue #177 is a *display*-math report, the tradeoff is one-sided. Display `\[` carries the same class of collision (`grep '\[foo\]'`) at far lower frequency, and is retained. Pinned by unit tests (sed, grep, `\|` alternation, the documentation sentence) and an e2e that asserts zero `.katex` for a message containing a sed command.

### DEC-8: During streaming, should a half-arrived `\[` be optimistically closed so math renders sooner?
**Resolution:** No. An unmatched opener passes through untouched and renders as plain text until the closer arrives.
**Basis:** convention — the regex requires a literal closer, so this is the natural behavior. Optimistically closing would feed KaTeX a truncated expression (`\frac{k`), which renders a red `katex-error` node; a flickering error block is worse than the plain text it replaces. Accepted cost: one frame of layout shift when the closer streams in.

### DEC-9: Does this feature introduce any operational tunable that must be admin-configurable?
**Resolution:** No. The feature introduces zero tunables — no limit, threshold, retention, quota, concurrency cap, toggle, or model selection. No settings row, migration, permission or sync entity is added.
**Basis:** convention — the configurable-settings rule applies to operational tunables; a delimiter grammar is a fixed correctness property of the markdown dialect, not an operator knob. Making "which delimiters mean math" configurable would be a footgun with no use case. The only numeric constants in the change are regex/parsing structure (e.g. the 4-space indented-code threshold), which are dictated by the CommonMark spec, not chosen.

### DEC-10: Where does the new pure module live?
**Resolution:** `src-app/ui/src/components/common/normalizeMathDelimiters.ts`.
**Basis:** codebase — `components/common/` already holds the cross-module markdown layer (`markdownPreprocess.ts`, `streamdownPlugins.ts`). Three of the five consumers are outside the chat module, so placing it under `modules/chat/core/utils/` (where `citationTokenize.ts` lives) would invert the dependency direction.

### DEC-11: Which test runner for the new unit tests?
**Resolution:** `node:test` (`npm run test:unit`), colocated `*.test.ts`.
**Basis:** codebase — `vitest.config.ts` is deliberately scoped to `src/**/*.store.test.ts` (jsdom, for Zustand stores needing `vi.mock`). Every pure-helper test in the tree (`citationTokenize.test.ts`, `collapsible.test.ts`, `conversationDisplayLabel.test.ts`) uses `node:test` + `node:assert/strict` with an explicit `.ts` import extension. Note `npm run check` does NOT run unit tests, so `npm run test:unit` is an explicit phase-8 step.
