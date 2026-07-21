// LLMs commonly write math with LaTeX's own delimiters — `\[ … \]` for display and
// `\( … \)` for inline — but remark-math (via @streamdown/math) is a micromark
// SYNTAX extension hard-wired to `$`, with no delimiter configuration. Markdown
// then eats the `\[` as a character escape, so the equation leaks through as a
// literal `[` followed by raw LaTeX (issue #177).
//
// The rewrite has to happen on the RAW STRING, before Streamdown parses: by the
// time any mdast plugin could run, tokenization has already mangled the LaTeX
// (`x_1 … y_1` consumed as emphasis, backslash escapes dropped). Same layer as the
// sibling `citationTokenize` / `preprocessMarkdown` pre-tokenizers.
//
// NOTE: this function does NOT skip code blocks — its caller owns that. It is
// invoked from `preprocessMarkdown`, inside the code-fence split loop that already
// protects fenced blocks and inline spans.

// Both delimiter pairs in ONE alternation so a single `lastIndex` walk consumes
// them left to right — that is what stops a `\(` sitting INSIDE an already-matched
// `\[ … \]` from being re-matched as inline math.
//
// The lookbehind guards the OPENER only, rejecting a doubly-escaped `\\[` (a
// literal backslash the model escaped) and the LaTeX row-break collision `a\\(b)`.
// The closer is deliberately UNguarded: `\[ x \\\]` — a row break immediately
// before the closer — is valid LaTeX and must still match. Over-matching is bounded
// by the blank-line guard below.
const MATH_RE = /(?<!\\)\\\[([\s\S]*?)\\\]|(?<!\\)\\\(([\s\S]*?)\\\)/g

// Leading indent, blockquote markers, then an optional list marker. Used to
// reconstruct the prefix every continuation line of a block needs in order to stay
// inside its container.
const CONTAINER_RE = /^([ \t]*)((?:> ?)*)([ \t]*)(?:([-*+]|\d{1,9}[.)])([ \t]+))?/

/**
 * The prefix each emitted line needs to remain in the same markdown container as
 * `lineHead`. Indent and blockquote markers carry over verbatim; a list marker
 * becomes equivalent-width spaces (a continuation line must align under the item's
 * content, not repeat the bullet).
 *
 * Returns `null` for an indented code block (4+ spaces with no bullet or quote),
 * which must never be touched.
 */
function continuationPrefix(lineHead: string): string | null {
  const m = CONTAINER_RE.exec(lineHead)
  if (!m) return ''
  const [, lead, bq, mid, marker, gap] = m
  // A tab counts as 4 columns of indentation in CommonMark, so `\t\[ x \]` is an
  // indented CODE block just as `    \[ x \]` is — measure columns, not chars.
  const indentColumns = lead.replace(/\t/g, '    ').length
  if (indentColumns >= 4 && !bq && !marker) return null
  return lead + bq + mid + (marker ? ' '.repeat(marker.length + gap.length) : '')
}

/**
 * Is the match sitting inside a link destination or title — `[t](http://x "…")`?
 * Injecting a newline there would break the link syntax outright (corruption, not
 * degradation), so those downgrade to inline math the same way a table row does.
 * True when the last `](` on the line has no `)` after it.
 */
function inLinkTarget(lineHead: string): boolean {
  const open = lineHead.lastIndexOf('](')
  return open !== -1 && !lineHead.includes(')', open + 2)
}

/**
 * Rewrite LaTeX-delimited math into the `$`-delimited forms remark-math parses:
 * `\[ … \]` → block math, `\( … \)` → inline math.
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
  if (md.indexOf('\\[') === -1 && md.indexOf('\\(') === -1) return md

  return md.replace(
    MATH_RE,
    (
      whole: string,
      display: string | undefined,
      inline: string | undefined,
      offset: number,
      str: string,
    ) => {
      const inner = (display ?? inline ?? '').trim()
      if (!inner) return whole

      // A blank line can't occur inside real math, but it CAN occur when an
      // unclosed `\[` runs on until some later `\]` — this bounds the damage to a
      // single paragraph instead of swallowing the rest of the message.
      // `\r?` throughout: an uploaded .md / SKILL.md may be CRLF.
      if (/\r?\n[ \t]*\r?\n/.test(inner)) return whole

      // Inline needs no block positioning. A `$` in the content would close the
      // span early and corrupt the rest of the line, so leave those alone.
      if (display === undefined) {
        return inner.includes('$') ? whole : `$${inner}$`
      }

      // A body line that is exactly `$$` would close the fence early.
      if (inner.split(/\r?\n/).some(line => line.trim() === '$$')) return whole

      const lineHead = str.slice(str.lastIndexOf('\n', offset - 1) + 1, offset)

      // Two places a newline cannot go: a table row (newline-terminated) and a
      // link destination/title (newline breaks the link syntax). Both downgrade
      // to inline math, which still renders and cannot corrupt the construct.
      if (lineHead.includes('|') || inLinkTarget(lineHead)) {
        return inner.includes('$') ? whole : `$${inner}$`
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
