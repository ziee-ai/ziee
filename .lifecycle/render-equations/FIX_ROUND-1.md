# FIX_ROUND-1 — render-equations

## Findings fixed from LEDGER.jsonl

- **correctness / CRLF** — the trailing-context test `/^[ \t]*(\n|$)/` did not match
  `\r\n`, so a CRLF document took the something-follows branch and emitted a stray
  prefix-plus-`\r` line after the closing fence. Fixed with `\r?` in the trailing
  test, the blank-line runaway guard, and the body split. Pinned by a new
  `CRLF input produces the same block structure as LF` test (4 assertions).

- **extensibility (medium, accepted)** — adding a future delimiter family means
  editing `MATH_RE`'s alternation rather than registering into a seam. Deliberately
  accepted for two delimiter pairs; a registry here would be more machinery than
  the thing it abstracts. Recorded in the ledger so the descoped `\begin{env}`
  follow-up (DEC-6) refactors rather than bolts on. No code change this round.

- The remaining `confirmed` ledger rows are not defects: the `$`-span row and the
  pipeline-verification row record VERIFIED BEHAVIOR (both are now pinned by
  tests), and the process-note row records how this phase was actually conducted.

## Re-audit round (adversarial, run not read)

Fifteen fresh adversarial inputs the first round had not covered were run through
both `normalizeMathDelimiters` and `preprocessMarkdown`: nested display-in-inline
and inline-in-display, two matches on one line (both kinds), an opener whose closer
sits in a later code fence, `\[` inside a link title, `\(` inside link text, math in
a table inside a list, ATX heading, setext underline, doubly-nested blockquote, tab
indent, whitespace-only inner, unicode body, and a stray closer before a real opener.

**Two NEW confirmed findings, both fixed:**

- **correctness — tab-indented code was not guarded.** A tab is 4 columns in
  CommonMark, so `\t\[ x \]` is an indented code block exactly as `    \[ x \]` is,
  but `continuationPrefix` measured `lead.length` in CHARACTERS (tab = 1) and
  converted it. Fixed by measuring columns (`lead.replace(/\t/g, '    ').length`).
  Plausible in real documents — tabs are common markdown indentation.

- **correctness — a `\[ … \]` inside a link destination/title was CORRUPTED.**
  `[t](http://x "\[ y \]")` became `[t](http://x "\n$$\ny\n$$\n")`, injecting
  newlines into the link syntax and breaking the link. This violated the design rule
  that every unhandled case must DEGRADE, never corrupt. Fixed with an
  `inLinkTarget(lineHead)` guard (last `](` with no `)` after it) that downgrades to
  inline math, the same treatment a table row already got. Pinned by two assertions
  including the negative case (a COMPLETED link earlier on the line must still allow
  a block conversion after it).

All other 13 probes behaved correctly. Notably: the alternation's single `lastIndex`
walk does prevent a nested `\(` inside a consumed `\[…\]` from re-matching; an opener
whose closer lies inside a later fence is left untouched by the blank-line guard; and
a stray `\]` before a real pair does not shift the match.

Known imperfections re-confirmed as acceptable (degrade, never corrupt): a setext
`=====` following a converted block no longer underlines its original title, and an
ATX heading containing display math splits into heading + block. Both are inherent to
display math being a block-level construct.

## Verification after fixes

- `src/components/common/*.test.ts`: 29/29 pass.
- Full `npm run test:unit`: 456 tests, 446 pass, 10 fail — the SAME 10 pre-existing
  failures present on a stashed clean tree (auth, voice ×4, scheduler ×2, chat
  stores ×3), all in modules untouched by this diff.
- `tsc --noEmit`: clean.

**New confirmed findings:** 0
