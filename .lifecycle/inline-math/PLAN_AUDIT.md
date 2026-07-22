# PLAN_AUDIT — inline math (Flavor B)

Audited against the codebase at `68af34059`, before writing code.

## Breakage risk

**Call-site inventory.** `normalizeMathDelimiters` has exactly ONE production caller —
`preprocessMarkdown` (`markdownPreprocess.ts:99`) — plus its own unit test. Verified by
repo-wide grep: the only other files naming either symbol are
`modules/chat/components/TextContent.tsx`,
`modules/chat/extensions/text/components/TextContent.tsx`,
`modules/file/viewers/markdown/body.tsx` (all calling `preprocessMarkdown`, not the
normalizer directly), the two unit tests, and the e2e spec. So the blast radius is
"every markdown surface that already renders assistant text and uploaded `.md` files" —
intended — and nothing else.

**Blocker found: the caller short-circuits inline input.** `markdownPreprocess.ts:75`
returns early when the document has no `[`, with the comment "`\[` contains `[`, so the
original guard already admits every input the math pass could act on — no widening
needed." True for display, **false for inline** — `\(` has no `[`. `Energy \( E = mc^2 \)
is nice.` would never reach the normalizer. Without ITEM-8 the feature is a silent no-op
on the most common real input, and every unit test would still pass (they call the
normalizer directly, bypassing the caller). This is exactly the class of gap a
green-on-paper suite hides; ITEM-8 fixes it and TEST-13 pins it at the `preprocessMarkdown`
level.

**Ordering hazard (resolved by ITEM-4).** The inline pass must run AFTER the display pass,
because display emits `$$` fences and an inline pass running first would rewrite a `\(`
that belongs inside a display body — producing `$$\na $b$ c\n$$`, which KaTeX cannot
parse. Running inline second, ITEM-4's paragraph guard sees the freshly-emitted `$$` and
skips, preserving the existing expectation `'\[ a \( b \) c \]'` → `'$$\na \( b \) c\n$$'`
byte-for-byte. This is load-bearing, not incidental — TEST-11 pins it.

**Regressions to pre-existing behavior.** `$…$`, `$$…$$`, and `\[…\]` handling is
untouched (the display `replace` is not modified). The one intentional behavior change is
that un-fenced `\(foo\)` prose now renders as math-italic instead of `(foo)` — the
accepted Flavor-B tradeoff, and no worse than markdown's pre-existing backslash strip.
Two existing tests pin the OLD behavior and must be rewritten, not deleted:
`normalizeMathDelimiters.test.ts` TEST-2 and the e2e
`leaves inline \( … \) untouched so sed/grep prose survives`. Rewriting a test that pins
superseded behavior is legitimate; A5 (no shrinkage) is respected because no TEST-ID is
removed.

**Performance.** This runs on every streaming frame. The inline matcher is bounded
single-line + 300 chars (ITEM-6), so each unmatched opener costs O(cap) not O(n) — same
reasoning as the display pass's 2000-char cap. ITEM-4's paragraph scan is bounded by the
containing paragraph and only runs for matches that already cleared the cheaper guards.

## Pattern conformance

The closest existing module is the **display pass in the same file** — the ideal
precedent, since it was written for this exact problem one PR ago. Conformance checks:

- Named module-level regex const with a rationale comment (lookbehind purpose, cap as a
  ReDoS bound not a style choice) — mirrored.
- Guard chain where every rejecting branch `return whole` (degrade-don't-corrupt) —
  mirrored; all six guards degrade to today's rendering.
- `\r?` wherever a newline is matched (CRLF-uploaded `.md`) — applies to ITEM-4's
  paragraph scan and ITEM-6's single-line matcher.
- Reuse `continuationPrefix` rather than re-deriving the indented-code-block test
  (ITEM-5) — no duplicated logic.
- `markdownPreprocess.ts:32` `MATH_SPAN_RE` already establishes the "math spans lines but
  not a blank line, be deliberately conservative" precedent that ITEM-4 follows.
- Unit tests go through the existing `check(input, expected)` helper so they inherit the
  `ALL_INPUTS` idempotence replay for free.
- e2e uses the file's existing `seedAssistantWithText` / `assistantBubble` helpers and its
  established convention of NOT asserting raw TeX is absent (KaTeX keeps it in a hidden
  `<annotation>`).

No shared harness (`tests/common/*`, gallery cassette, playwright configs) is touched —
B3 not in play. No new component, token, testid, or render state, so
`check:kit-manifest` / `check:testid-registry` / `check:design-spec` /
`check:state-matrix` / `check:gallery-coverage` have nothing new to satisfy.

## Migration collisions

**None.** No migration is added. Server migrations are module-owned under
`src-app/server/src/modules/*/migrations/` (merged by `build.rs::compose_merged_migrations`);
desktop's highest is `10000000000005_create_host_mounts.sql`. The diff does not touch
`src-app/server/**` or `src-app/desktop/tauri/**` at all, so a collision is structurally
impossible. See BASE.md.

## OpenAPI regen

**Not required.** No Rust type, route, request/response shape, or permission changes.
Neither `openapi.json` nor `api-client/types.ts` is touched in `src-app/ui` or
`src-app/desktop/ui`, so C3 regen-parity is trivially satisfied and the phase-6 coverage
exclusion for generated files is not engaged.

**R2-3 desktop-override check (SECURITY).** `src-app/desktop/ui/` carries hand-written
overrides of `src-app/ui/`. Verified by `find` over `src-app/desktop/ui/src`: there is no
override of `normalizeMathDelimiters.ts` or `markdownPreprocess.ts`, so desktop consumes
the shared implementation and no parallel edit is needed. Re-confirm before phase 8 if the
diff grows.

**A8 / A9 / A10 (MCP + permission gates):** not engaged — no built-in MCP server, no
permission introduced, no `modules/*/permissions.rs` or migration grant in the diff.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — mirrors the display pass in the same file; single
  production call site confirmed; ordering (display first, inline second) is required and
  is pinned by TEST-11.
- **ITEM-2** — verdict: PASS — the nested-opener and body-`$` guards are direct analogues
  of the display path's existing `inner.includes('\\[')` guard and `asInlineMath`'s `$`
  check.
- **ITEM-3** — verdict: PASS — pure additive skip; strictly reduces the set of converted
  matches, so it cannot introduce corruption. Keeps `\(a\|b\)` byte-identical to today.
- **ITEM-4** — verdict: PASS — the security/correctness core of the change. "Any
  un-escaped `$` in the paragraph" is deliberately stronger than an odd-count test,
  because `$5 and $10 for \( E \)` is even-count and still hijacks. Cost is
  under-conversion in mixed `$`+`\(` paragraphs, which is degrade-not-corrupt and
  therefore acceptable. Empirically grounded in the micromark runs recorded in PLAN.md.
- **ITEM-5** — verdict: PASS — reuses `continuationPrefix`; no new logic. Note the guard
  is defence-in-depth: `preprocessMarkdown` already splits out fenced and inline code, so
  this covers only the 4-space/tab indented-code form, which that split does not handle.
- **ITEM-6** — verdict: PASS — carries over the display pass's own ReDoS/streaming
  reasoning. The single-line restriction is strictly stronger than display's blank-line
  guard and is the right call for inline (a real inline equation never spans a line).
- **ITEM-7** — verdict: PASS — required, not cosmetic: both comments currently assert the
  opposite of what the code will do, and a stale comment asserting a safety property is
  worse than no comment.
- **ITEM-8** — verdict: CONCERN — **found during this audit, not in the original plan.**
  Without it ITEM-1 is a no-op for any message lacking a `[`, and the unit tests cannot
  detect that because they call the normalizer directly. Mitigation: implement the
  widening AND add TEST-13, which asserts through `preprocessMarkdown` (the real entry
  point) rather than the normalizer. Resolved by amending PLAN.md; no `BLOCKED` verdicts
  remain.
