# INFRA_INTEGRATION — render-equations

The three mandatory per-item walks. This feature is a pure render-path change: no
entity, no persistence, no permission, no network call, no new state.

## 1. User-experience walk

**How a real user encounters this.** They ask a technical question; the model
answers with equations. Today, roughly half the time (whenever the model picks
LaTeX delimiters over `$`), they see `[ \frac{d^2C(x)}{dx^2} - \frac{k}{D}C(x) = 0 ]`
— unreadable. The delimiter choice is the model's, not theirs, so from the user's
side the failure looks random: the same question can render correctly or not
depending on which convention the model happened to emit. After this change both
conventions render.

Per item:

- **ITEM-1/2/3** — the user reads a formatted equation instead of raw LaTeX. Display
  math is centered on its own line (`.katex-display`), inline math sits in the
  sentence. Verified by running the real remark/rehype pipeline (11 cases, incl.
  the two literal #177 equations) and by TEST-7.
- **ITEM-4** — the user never sees something *worse* than today. Every guard is a
  return-unchanged, so an input the transform can't handle safely renders exactly
  as it does now. The one non-identity guard (table row → inline math) still
  renders math, just not centered.
- **ITEM-5** — invisible to the user by design; the whole point is that non-math
  markdown is unchanged. Proven byte-identical across a 26-input corpus by running
  the pre-change implementation side-by-side with the reworked one.
- **ITEM-6** — assistant chat messages gain reference-link inlining and blocked-image
  placeholders, matching what the sibling renderer already did. A user who had a
  reference-style link render as raw `[text][id]` now sees a working link.
- **ITEM-7** — skill drawer and workflow step output gain all three passes. A user
  reading a skill whose SKILL.md contains math now sees it rendered.
- **ITEM-8** — no user-facing effect; corrects a test that contradicted shipped code.

**Streaming is part of the UX.** While an equation is arriving the user sees plain
text (today's behavior), which snaps to a rendered block when the closing delimiter
lands. That is one layout shift per equation. Optimistic closing was considered and
rejected (DEC-8) because feeding KaTeX `\frac{k` produces a red error node that
flickers worse than the plain text it replaces.

## 2. Infrastructure-integration walk

Every subsystem the render path touches, and whether it has behavior this change
must handle rather than assume:

| Subsystem | Interaction | Handled |
|---|---|---|
| **Streaming / SSE** | `TextContent` re-renders on every `text_delta`; `preprocessMarkdown` runs per frame | Pure function, `O(n)`, two `indexOf` early-outs. An unmatched opener cannot match, so partial frames are byte-identical passthrough. No state carried between frames, so no torn intermediate. |
| **Streamdown incremental parse** | Streamdown parses block-by-block for streaming | The transform runs BEFORE Streamdown sees the string, so Streamdown's own incomplete-markdown handling is unaffected. It has no `$`/`\[` special-casing to conflict with (verified: zero matches in its dist). |
| **`citationTokenize`** | Runs before `preprocessMarkdown` in both chat renderers | Composition order preserved (`citationTokenize` → `preprocessMarkdown`). Citation output is `[n](#kb-cite-n)` — contains no `\[`/`\(`, so the math pass cannot see it. Conversely math output is `$…$` — contains no bare `[n]`, so it cannot manufacture a citation. Independently, the nested math-span split keeps the reference passes out of math. |
| **Code / Shiki plugin** | Fenced + inline code must stay literal | Owned by `preprocessMarkdown`'s pre-existing code-fence split; the math pass runs strictly INSIDE that loop's even segments. Proven by TEST-6 and TEST-8. |
| **Mermaid renderer** | `renderers: [{ language: 'mermaid', … }]` | Mermaid arrives as a fenced block → protected by the same code split. Untouched. |
| **KaTeX / rehype-katex** | Consumes the `$` forms | Already wired and already shipping; this change only produces MORE of what it already consumes. `errorColor` default already routes a bad expression to a styled error rather than a crash. |
| **`StreamdownErrorBoundary`** | Wraps all 5 call sites | Unchanged. A malformed transform result would be caught here, but the transform can only ever emit `$`, newline and the original text. |
| **`useStreamdownComponents`** | `a`/`img`/`li` overrides, footnote scoping | Untouched. The `a` override's `isCitationHref` check is unaffected (math emits no anchors). |
| **File markdown viewer** | Already called `preprocessMarkdown` | Inherits math with no edit — the shared-pipeline dividend. |
| **Permissions / authz** | — | No permission introduced or checked. A9/A10 are N/A. |
| **Sync / SSE entities** | — | No entity, no `sync:` emission, no store. |
| **Persistence / migrations** | — | None. The transform is display-only; the stored message text is never rewritten, so turning the feature off would restore the old render exactly. |
| **OpenAPI / api-client** | — | No route or type change; no regen. |
| **Desktop (`src-app/desktop/ui`)** | — | Does not mirror any touched file (no `TextContent`, no Streamdown call site, no `streamdownPlugins`). R2-3 desktop-override review is N/A. |

**The one non-obvious constraint this walk surfaced**: `isSameOriginImage` reads
`window.location.origin`, which does not exist under the `node:test` runner. It is
inside a `try/catch` that returns `false`, so unit tests are deterministic but treat
every absolute URL as external. Recorded in `markdownPreprocess.test.ts` rather than
papered over with a `window` stub; browser-faithful image behavior is covered by e2e.

## 3. Entity-lifecycle walk

**This feature holds no entity.** There is no row, no cache, no store field, no
subscription, no id. `normalizeMathDelimiters` and `preprocessMarkdown` are pure
`string → string` functions with no closure state, no module-level mutable state,
and no side effects.

Walking the required matrix explicitly, for the only thing the feature "holds" —
the message/document text it is handed as a prop:

| Event | Local path | Sync/SSE path |
|---|---|---|
| **Add** | New text arrives as a prop → transformed on render | Same — a synced message renders through the identical function |
| **Mutate** (streaming delta) | Re-render with the new buffer; no memo keyed on old text | Same |
| **Remove / delete** | The component unmounts with its parent; nothing to clean up | Same — no subscription to leak |
| **Access-loss** | The surface unmounts via existing permission gating; the transform is never reached | Same |

There is deliberately **no memoization** of the transform result. A cache keyed on
message id would be exactly the stale-on-delete hazard this walk exists to catch;
the function is cheap enough (two `indexOf` early-outs, then one pass) that caching
would add a lifecycle bug to save nothing measurable.

The two module-level regexes carry the `g` flag, which makes `lastIndex` stateful on
the *regex object*. `MATH_RE` is only ever used via `String.prototype.replace`, which
resets `lastIndex` itself — but note the pre-existing `DEF_RE` in `markdownPreprocess`
must, and does, reset `lastIndex = 0` explicitly before its `exec` loop. No new
`exec` loop was introduced.
