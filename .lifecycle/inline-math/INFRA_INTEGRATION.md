# INFRA_INTEGRATION — inline math (Flavor B)

The three mandatory phase-5 walks, performed per item while implementing.

## 1. User-experience walk

**Who hits this and how.** A user asks a quantitative question. The model replies with
prose carrying inline math in LaTeX's own delimiters — `the decay length \( \lambda =
\sqrt{D/k} \)`, `the coefficient \( D \)`, `where \( C(x) \) is bounded`. This is not
occasional: it is what essentially every model emits for inline math unless prompted
otherwise, which is why a prompt-level fix was rejected as non-generalizing.

**Before.** Markdown's own character-escape rule strips the backslashes and the user reads
`the decay length ( \lambda = \sqrt{D/k} )` — parentheses where an equation should be,
and raw `\lambda` / `\sqrt{}` control sequences leaking into prose. Display math (`\[ … \]`)
was fixed in #188; inline was the visibly-broken remainder in the same reply.

**After.** The span renders as inline KaTeX, flowing with the sentence, with the TeX source
preserved in KaTeX's hidden `<annotation>` element for screen readers (the a11y win, and
the reason the existing e2e specs deliberately do NOT assert raw TeX is absent from the
DOM).

**Living with it — streaming.** Text arrives frame by frame and the whole preprocessor
re-runs per frame. A half-arrived `\( E=` does not match (no closer), so it renders as
literal text and then snaps to an equation when the closer arrives. That is a visible
one-frame transition, but it is exactly the behavior display math has shipped with since
#188, so it is consistent rather than novel. The degrade is always "renders as today",
never a corrupted intermediate.

**Living with it — the tradeoff the user chose.** Prose like `sed -e 's/\(foo\)/bar/'`
written OUTSIDE a code block now renders with an italic math `foo`. The user is not losing
information they previously had: markdown already ate the backslashes, so they were already
reading `s/(foo)/bar/`, never the literal source. A real command belongs in a code block,
which stays byte-literal (TEST-14, TEST-17).

## 2. Infrastructure-integration walk

Every existing subsystem this change touches, and the specific constraint each imposes.

| Subsystem | Constraint found | How it is handled |
|---|---|---|
| **`preprocessMarkdown` early return** | Bails on any document with no `[`. `\[` has one, `\(` does not — so inline-only messages never reached the math pass at all. | ITEM-8 widens the guard. This was the single highest-impact finding of the whole feature: without it every unit test still passes and the feature is a user-facing no-op. Pinned by TEST-13 (which asserts through `preprocessMarkdown`, not the normalizer). |
| **Code-fence / inline-code split** | The guards inside `normalizeMathDelimiters` cannot see code structure; ONLY the caller's `md.split(/```…|~~~…|`…`/)` protects code, and it rewrites EVEN indices only. | Unchanged and relied upon. Its importance rises sharply under Flavor B (a fenced `sed 's/\(foo\)/bar/'` is now the difference between literal and math), so TEST-14 and TEST-17 pin it at the `preprocessMarkdown` and DOM levels respectively — not at the normalizer level, where it is not provable. |
| **`MATH_SPAN_RE` protection (passes 1 & 2)** | The reference-link and image passes must never reach inside a math span, or `$$ a[1] $$` gets its `[1]` rewritten into a link. | Automatically correct: `normalizeMathDelimiters` runs FIRST and its output is then split on `MATH_SPAN_RE`, whose `\$[^$\n]*\$` alternative matches the single-dollar spans this feature newly produces. So a `\( a[1] \)` → `$a[1]$` is protected from the bracket passes exactly as a display span already was. No change needed — verified by reading the split, not assumed. |
| **`citationTokenize`** | Runs BEFORE `preprocessMarkdown` (`preprocessMarkdown(citationTokenize(text))` in both chat `TextContent`s), rewriting bare `[n]` into citation links. | Pre-existing ordering, unchanged. A `\( a[1] \)` therefore has its `[1]` tokenized before the math pass sees it — but that is identical to the pre-existing behavior for `\[ a[1] \]`, is not a regression introduced here, and `citationTokenize` has its own code-span protection. Left alone deliberately; changing pre-tokenizer order would be an unrelated, riskier change. |
| **Streaming chat pipeline** | The preprocessor runs on EVERY frame, so per-call cost compounds over a response. | ITEM-6's single-line + 300-char cap makes each unmatched opener O(cap) rather than O(n), mirroring the display pass's documented 2000-char bound. The paragraph scan (the only guard that looks beyond the match) runs last, after the cheap guards have already rejected most candidates. |
| **`remark-math` / `singleDollarTextMath`** | Inline `$…$` only parses because `streamdownPlugins.ts:35` passes `singleDollarTextMath: true`. If that were ever turned off, this feature would emit `$…$` that renders as literal dollars. | Verified enabled at the single plugin-construction site. Noted here as the one external precondition; it is also what the display pass's `asInlineMath` downgrade already depends on, so the dependency is shared, not new. |
| **File viewer (`viewers/markdown/body.tsx`)** | Renders uploaded `.md`, which may be CRLF-authored. | The inline matcher excludes `\r` as well as `\n` (`[^\r\n]`), so a CRLF document cannot produce a body with a trailing `\r`; the paragraph scanners trim candidate lines, so a CRLF blank line is still recognised as a paragraph break. |
| **Desktop UI overrides (R2-3)** | `src-app/desktop/ui/` carries hand-written overrides that can silently drop shared logic. | Verified by `find` over `src-app/desktop/ui/src`: no override of `normalizeMathDelimiters.ts` or `markdownPreprocess.ts` exists. Desktop consumes the shared implementation; no parallel edit needed. |
| **Skill / workflow surfaces** | DEC-10 keeps them out of scope. | Verified by grep: `preprocessMarkdown` has exactly THREE production call sites — `modules/chat/components/TextContent.tsx`, `modules/chat/extensions/text/components/TextContent.tsx`, `modules/file/viewers/markdown/body.tsx`. A comment in `markdownPreprocess.test.ts:39-41` claims the skill drawer and workflow step output also use it; that claim is **stale** — neither imports it. So scope is satisfied structurally, not by restraint. |
| **Permissions / sync / MCP / notifications / settings** | — | Not engaged. No permission, sync entity, MCP server, notification, settings row, migration, or API is involved (DEC-9). A8/A9/A10 are structurally inapplicable. |

## 3. Entity-lifecycle walk

This feature holds, caches, and owns **no entity**. It is a pure, synchronous
`string → string` function with no state, no store, no subscription, no persistence, and
no identity: given the same input it returns the same output, and it retains nothing
between calls.

Consequently the add / remove / **delete** / mutate / access-loss matrix, and the
local-vs-sync(SSE) dual-path requirement, have no subject here. The entity whose lifecycle
matters — the message — is owned by the chat stores and is entirely unaffected: this code
runs at render time on whatever text it is handed, so a message being edited, deleted,
regenerated, or arriving over sync simply causes a re-render with different input. There is
no cached derivation that could go stale, and nothing to invalidate.

The one lifecycle-adjacent property that DOES matter is **idempotence under repeated
application** (the same text is re-processed on every streaming frame and every re-render).
That is proven, not assumed: neither pass's output can contain `\[` or `\(`, and TEST-12
replays every input in the suite through the function twice and asserts a fixed point.
