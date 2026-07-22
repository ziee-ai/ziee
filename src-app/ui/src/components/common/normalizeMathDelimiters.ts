// LLMs commonly write DISPLAY math with LaTeX's own delimiters — `\[ … \]` — but
// remark-math (via @streamdown/math) is a micromark SYNTAX extension hard-wired to
// `$`, with no delimiter configuration. Markdown then eats the `\[` as a character
// escape, so the equation leaks through as a literal `[` followed by raw LaTeX
// (issue #177).
//
// The rewrite has to happen on the RAW STRING, before Streamdown parses: by the
// time any mdast plugin could run, tokenization has already mangled the LaTeX
// (`x_1 … y_1` consumed as emphasis, backslash escapes dropped). Same layer as the
// sibling `citationTokenize` / `preprocessMarkdown` pre-tokenizers.
//
// INLINE `\( … \)` IS DELIBERATELY NOT CONVERTED. It is byte-identical to POSIX
// BRE group syntax, so converting it corrupts ordinary prose that no reader would
// consider math:
//
//     sed -e 's/\(foo\)/bar/'      would become   sed -e 's/$foo$/bar/'
//     Pattern \(a\|b\) matched     would become   Pattern $a\|b$ matched
//     To escape use \( and \).     would become   To escape use $and$.
//
// and there is no structural rule that separates those from a real `\( E=mc^2 \)`
// — the last example is whitespace-padded exactly like genuine math. Since inline
// math already renders via `$…$` (singleDollarTextMath is enabled), the tradeoff
// is one-sided: convert display only, and never risk mangling prose. Issue #177
// is a display-math report.

// Display delimiters only. The lookbehind guards the OPENER, rejecting a
// doubly-escaped `\\[` (a literal backslash the model escaped). The closer is
// deliberately UNguarded: `\[ x \\\]` — a LaTeX row break immediately before the
// closer — is valid and must still match.
//
// The length cap is a ReDoS bound, not a style choice. An unbounded lazy
// `[\s\S]*?` rescans to end-of-string for EVERY unmatched opener, which is
// quadratic in document size — and because this runs on every streaming frame it
// compounds again over a response. Capping the body makes each scan O(cap) rather
// than O(n): measured 4x-per-doubling before the cap, 2x after. 2000 chars is far
// beyond any real display equation; a longer one simply doesn't convert, which is
// the same degrade-don't-corrupt contract every other guard follows.
const MATH_RE = /(?<!\\)\\\[([\s\S]{0,2000}?)\\\]/g

// Leading indent, blockquote markers, then an optional list marker. Used to
// reconstruct the prefix every continuation line of a block needs in order to stay
// inside its container. The 1-9 digit bound is CommonMark's ordered-list-marker
// limit.
const CONTAINER_RE = /^([ \t]*)((?:> ?)*)([ \t]*)(?:([-*+]|\d{1,9}[.)])([ \t]+))?/

/**
 * The prefix each emitted line needs to remain in the same markdown container as
 * `lineHead`. Indent and blockquote markers carry over verbatim; a list marker
 * becomes equivalent-width spaces (a continuation line must align under the item's
 * content, not repeat the bullet).
 *
 * Returns `null` for an indented code block (4+ columns with no bullet or quote),
 * which must never be touched.
 */
function continuationPrefix(lineHead: string): string | null {
  // CONTAINER_RE is anchored and every group is optional, so exec always matches.
  const [, lead, bq, mid, marker, gap] = CONTAINER_RE.exec(lineHead)!
  // A tab counts as 4 columns of indentation in CommonMark, so `\t\[ x \]` is an
  // indented CODE block just as `    \[ x \]` is — measure columns, not chars.
  const indentColumns = lead.replace(/\t/g, '    ').length
  if (indentColumns >= 4 && !bq && !marker) return null
  return lead + bq + mid + (marker ? ' '.repeat(marker.length + gap.length) : '')
}

/**
 * Is the match sitting inside a link destination or title — `[t](http://x "…")`?
 * Injecting a newline there would break the link syntax outright (corruption, not
 * degradation). True when the last `](` on the line has no `)` after it.
 */
function inLinkTarget(lineHead: string): boolean {
  const open = lineHead.lastIndexOf('](')
  return open !== -1 && !lineHead.includes(')', open + 2)
}

/** Emit inline math, unless a `$` in the body would close the span early. */
const asInlineMath = (inner: string, whole: string): string =>
  inner.includes('$') ? whole : `$${inner}$`

/**
 * Rewrite LaTeX display delimiters `\[ … \]` into the `$$ … $$` block form that
 * remark-math parses. Inline `\( … \)` is passed through UNCHANGED — see the
 * module header for why.
 *
 * Two parser mechanics drive the block form, both verified against the installed
 * micromark-extension-math rather than assumed:
 *
 * 1. `$$x$$` on ONE line is parsed as *inline* math (`math-inline`). Display math
 *    requires the content on its own line — hence `$$\n…\n$$`.
 * 2. A `$$` at a line start DOES interrupt an open paragraph (`mathFlow` is a
 *    concrete flow construct), so a single `\n` is enough to reach block position.
 *    No blank line is needed, which is what lets the block nest inside a list item
 *    or blockquote without terminating it.
 *
 * Every guard degrades to "leave the text exactly as it is" — today's behavior —
 * so a case this can't handle safely renders as it does now, never corrupted. In
 * particular an unclosed `\[` simply fails to match, which is what makes the
 * function safe to run on every frame of a streaming response.
 */
export function normalizeMathDelimiters(md: string): string {
  if (typeof md !== 'string') return md
  if (md.indexOf('\\[') === -1) return md

  return md.replace(
    MATH_RE,
    (whole: string, display: string, offset: number, str: string) => {
      const inner = display.trim()
      if (!inner) return whole

      // A blank line can't occur inside real math, but it CAN occur when an
      // unclosed `\[` runs on until some later `\]` — this bounds the damage to a
      // single paragraph instead of swallowing the rest of the message.
      // `\r?` throughout: an uploaded .md may be CRLF.
      if (/\r?\n[ \t]*\r?\n/.test(inner)) return whole

      // A body line that is exactly `$$` would close the fence early.
      if (inner.split(/\r?\n/).some(line => line.trim() === '$$')) return whole

      // A nested `\[` means the lazy closer matched the INNER `\]`, so converting
      // would emit a block plus a dangling literal `\]`. Not valid LaTeX anyway —
      // leave the whole thing alone rather than produce unbalanced output.
      if (inner.includes('\\[')) return whole

      const lineHead = str.slice(str.lastIndexOf('\n', offset - 1) + 1, offset)

      // Two places a newline cannot go: a table row (newline-terminated) and a
      // link destination/title (a newline breaks the link syntax). Both downgrade
      // to inline math, which still renders and cannot corrupt the construct.
      if (lineHead.includes('|') || inLinkTarget(lineHead)) {
        return asInlineMath(inner, whole)
      }

      const prefix = continuationPrefix(lineHead)
      if (prefix === null) return whole

      const body = inner
        .split(/\r?\n/)
        .map(line => prefix + line.trim())
        .join('\n')
      // Only break the line when something precedes the match on it.
      const open = lineHead.trim() === '' ? '' : `\n${prefix}`
      // ...and only after it when something follows on the same line. The `\r?`
      // matters: without it a CRLF document takes the something-follows branch
      // and emits a stray prefix-plus-`\r` line after the closing fence.
      const trailing = str.slice(offset + whole.length)
      const close = /^[ \t]*(\r?\n|$)/.test(trailing)
        ? `\n${prefix}$$`
        : `\n${prefix}$$\n${prefix}`

      return `${open}$$\n${body}${close}`
    },
  )
}
