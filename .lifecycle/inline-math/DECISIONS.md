# DECISIONS — inline math (Flavor B)

Every human/product input the implementation needs, resolved before writing code.

### DEC-1: Which conversion flavor — content-gated (math-signal heuristic) or aggressive (convert everything)?
**Resolution:** Aggressive (Flavor B). Convert EVERY `\( … \)` clearing the guards; no
content/math-signal gate. Bare function notation (`\(C(x)\)`, `\(f(x)\)`) and bare
symbols (`\(D\)`) therefore render.
**Basis:** user — presented both flavors with before→after tables, the three regression
cases and a recommendation of the content-gated variant; the human explicitly chose B for
maximum coverage across all models, accepting that `s/\(foo\)/bar/` prose becomes
math-italic (no worse than markdown's pre-existing backslash strip).

### DEC-2: Do the BRE/regex guards stay under Flavor B?
**Resolution:** Yes. `\|`, `\1`–`\9`, `\{`, `\}`, `\+`, `\?` in the body skip conversion.
**Basis:** user — explicitly directed ("KEEP guard 4"). Also principled: these are
unambiguous regex syntax with no LaTeX-math reading, so skipping them costs nothing and
keeps `Pattern \(a\|b\)` byte-identical to today.

### DEC-3: What scope does the unpaired-`$` guard use — line, paragraph, or whole document?
**Resolution:** Paragraph (blank-line-delimited region containing the match).
**Basis:** codebase + empirical. Verified against the installed micromark that a `$…$`
span DOES cross a plain newline but does NOT cross a blank line, so a line-scoped guard
would miss a real hijack (`cost $5 line one⏎and \( E \) here` corrupts). Document scope
would be needlessly destructive — one price mention would disable inline math for an
entire long answer. Paragraph scope matches the precedent reasoning already documented on
`MATH_SPAN_RE` (`markdownPreprocess.ts:32`).

### DEC-4: "Any `$` in the paragraph" or "an ODD number of `$`"?
**Resolution:** Any un-escaped `$` outside the match → skip.
**Basis:** convention (degrade-don't-corrupt, the contract every existing guard in this
file follows). An odd-count test is provably insufficient: `$5 and $10 for \( E \)` has an
even count and still hijacks, because micromark pairs left-to-right. "Any" is the only
rule that cannot change how pre-existing `$` tokens pair. Accepted cost: under-conversion
in paragraphs mixing `$` with `\(`, which is a degrade, not a corruption.

### DEC-5: Does an escaped `\$` count toward the guard?
**Resolution:** No — `\$` is skipped when scanning for blocking dollars.
**Basis:** empirical — verified that `cost \$5 and $E$ here` renders `E` as math and `$5`
as literal text, i.e. an escaped `$` never opens a math span and therefore cannot hijack.
Counting it would suppress conversion for no benefit.

### DEC-6: May an inline body span multiple lines?
**Resolution:** No. The matcher is `[^\r\n]{0,300}?` — single line only.
**Basis:** convention + risk. A real inline equation never spans a line, while allowing
newlines would let an unclosed `\(` run away and swallow arbitrary downstream text
(exactly the failure the display path's blank-line guard exists to bound). Single-line is
the stronger and simpler form of that same guard.

### DEC-7: What is the body length cap, and is it a fixed constant or admin-configurable?
**Resolution:** Fixed constant, 300 characters, expressed inline in the named
`INLINE_MATH_RE` const with a comment stating it is a ReDoS bound.
**Basis:** convention — this is not an operational tunable (no resource limit, retention,
quota, or feature toggle an operator would ever want to change); it is a parser safety
bound, directly mirroring the display pass's existing 2000-char cap in the same file,
which is likewise a fixed constant. Promoting it to a settings row would add a DB read to
a per-streaming-frame string function for zero operator value. 300 is ~6× the longest
plausible inline equation (`\( C(x) = C_0 e^{-x/\lambda} \)` is 31 chars) while keeping
each unmatched-opener rescan cheap.

### DEC-8: Should the two existing tests that pin the OLD pass-through behavior be deleted or rewritten?
**Resolution:** Rewritten in place to assert the new Flavor-B behavior, keeping their
TEST-IDs and adding the `\(a\|b\)` guard-4 assertion so the file still proves the regex
case is protected.
**Basis:** user — explicitly directed ("Rewrite the existing sed/escape-prose unit + e2e
tests to assert the NEW B behavior … instead of unchanged"). Consistent with A5: no
TEST-ID is removed, so enumerated coverage does not shrink.

### DEC-9: Does this feature introduce any operational tunable, permission, or settings surface?
**Resolution:** No. No permission, no settings row, no migration, no API, no feature
toggle. The only constant introduced is the DEC-7 parser bound.
**Basis:** codebase — the change is confined to two pure string functions in
`src-app/ui/src/components/common/`. Consequently the mandatory configurable-settings rule
is satisfied by explicit non-applicability, and gates A8/A9/A10 are not engaged (see
PLAN_AUDIT.md).

### DEC-10: Should the fix also cover the skill and workflow markdown surfaces?
**Resolution:** No — out of scope, unchanged from #188.
**Basis:** user — the task brief states "Do NOT touch skill/workflow surfaces (kept out of
scope in #188)."

### DEC-11: Does `src-app/desktop/ui/` need a parallel edit?
**Resolution:** No.
**Basis:** codebase — verified by `find` over `src-app/desktop/ui/src` that no
hand-written override of `normalizeMathDelimiters.ts` or `markdownPreprocess.ts` exists;
desktop consumes the shared `src-app/ui` implementation. R2-3 discharged.

### DEC-12: Where does the inline pass live — a new module or the existing file?
**Resolution:** The existing `normalizeMathDelimiters.ts`, as a second `.replace` inside
the exported function, after the display pass.
**Basis:** user + convention — the task brief says "Extend the existing
`normalizeMathDelimiters.ts` (add the inline branch)". It also keeps the ordering
dependency (display must run first — see PLAN_AUDIT.md) local and enforceable in one
function rather than spread across two modules with an implicit call-order contract.
