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
// INLINE `\( … \)` IS ALSO CONVERTED, in a second pass. The original version of
// this module refused to, on the theory that `\(foo\)` is byte-identical to POSIX
// BRE grouping and converting it would corrupt shell prose. Measured against the
// installed micromark, that premise does not hold: in UN-FENCED prose markdown's
// own character-escape rule ALREADY strips `\(` → `(` before any math logic runs.
//
//     Ran: sed -e 's/\(foo\)/bar/'   renders TODAY as   Ran: sed -e 's/(foo)/bar/'
//     Pattern \(a\|b\) matched       renders TODAY as   Pattern (a|b) matched
//     To escape use \( and \).       renders TODAY as   To escape use ( and ).
//     energy \( E = mc^2 \) here     renders TODAY as   energy ( E = mc^2 ) here
//
// The backslashes are lost either way, so converting cannot make those cases
// MEANINGFULLY worse — it trades `(foo)` for a math-italic `foo`. Meanwhile the
// only correct home for a real sed/regex command is a code block, and the caller
// (`preprocessMarkdown`) already splits on fences + inline code before this runs,
// so genuine code is never reached. Models emit inline math constantly
// (`\( E=mc^2 \)`, `\( \lambda \)`, `\( D \)`, `\( C(x) \)`); leaving it raw was
// the larger, far more common defect.
//
// THE ONE REAL CORRUPTION VECTOR is not prose — it is an unpaired `$` nearby.
// Injecting a `$` delimiter into text that already contains a lone `$` lets the
// two pair up:
//
//     cost $5 and \( E \) here   ->   cost $5 and $E$ here
//                                     ...which renders as math "5 and " followed
//                                     by a dangling literal "E$ here".
//
// The display pass is immune (a `$$` run cannot pair with a lone `$`); the inline
// pass is not. `paragraphBlocksInlineMath` below is the guard, and it is the
// reason this pass runs SECOND — by then the display pass has emitted its `$$`
// fences, so a `\(` sitting INSIDE a display body is recognised as already being
// in a math span and left alone, rather than rewritten into KaTeX-unparseable
// `$$\na $b$ c\n$$`.
//
// The guard pairs `$` runs BY LENGTH, the way micromark does, instead of merely
// counting them. That distinction is what lets a display block and inline math
// coexist in one paragraph: the display pass emits `$$` with single newlines (so
// the block stays inside its list item / blockquote), which leaves it in the SAME
// blank-line-delimited paragraph as the prose around it — and a `$$` run can
// never close the single `$` we emit.

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

// Inline delimiters. Same doubly-escaped-opener lookbehind as the display form,
// and the same degrade-don't-corrupt contract.
//
// The body is SINGLE-LINE by construction (`[^\r\n]`), which is stricter than the
// display path's blank-line guard and deliberately so: a real inline equation
// never spans a line, while allowing newlines would let one unclosed `\(` run on
// to some later `\)` and swallow arbitrary downstream text. The 300-char cap is
// the same ReDoS bound the display path documents — this runs on every frame of a
// streaming response, so each unmatched opener must cost O(cap), not O(n). 300 is
// ~6x the longest plausible inline equation.
const INLINE_MATH_RE = /(?<!\\)\\\(([^\r\n]{0,300}?)\\\)/g

// POSIX BRE / ERE syntax with no LaTeX-math reading: alternation `\|`, a
// backreference `\1`-`\9`, an interval `\{n,m\}`, `\+`, `\?`. A body carrying any
// of these is regex, not an equation, so it is left exactly as it renders today.
// This does cost the rare genuine `\( \{1,2\} \)` (set braces) and `\( \|v\| \)`
// (norms), which stay literal — a deliberate degrade, never a corruption.
const BRE_SIGNAL_RE = /\\[|{}+?1-9]/

/**
 * Does `s` contain a `$` that markdown would treat as LIVE (able to open or close
 * a math span)? A `$` is escaped only when preceded by an ODD number of
 * backslashes — `\$` is escaped, `\\$` is a literal backslash followed by a live
 * `$`. A lookbehind can't count, hence the loop.
 */
function hasLiveDollar(s: string): boolean {
  for (let i = s.indexOf('$'); i !== -1; i = s.indexOf('$', i + 1)) {
    let slashes = 0
    while (i - slashes - 1 >= 0 && s[i - slashes - 1] === '\\') slashes++
    if (slashes % 2 === 0) return true
  }
  return false
}

/**
 * Index of the first character of the blank-line-delimited paragraph containing
 * `from`. Paragraph, not line: verified against micromark that a `$…$` span DOES
 * cross a plain newline but does NOT cross a blank line, so a line-scoped guard
 * would miss a real hijack. `\r?` is handled by trimming the candidate line.
 */
function paragraphStart(str: string, from: number): number {
  let cursor = from
  while (cursor > 0) {
    const nl = str.lastIndexOf('\n', cursor - 1)
    if (nl === -1) return 0
    const prev = str.lastIndexOf('\n', nl - 1)
    if (str.slice(prev + 1, nl).trim() === '') return nl + 1
    cursor = nl
  }
  return 0
}

/** Index just past the last character of that paragraph. Mirror of the above. */
function paragraphEnd(str: string, from: number): number {
  let cursor = from
  while (cursor < str.length) {
    const nl = str.indexOf('\n', cursor)
    if (nl === -1) return str.length
    const next = str.indexOf('\n', nl + 1)
    if (str.slice(nl + 1, next === -1 ? str.length : next).trim() === '') return nl
    cursor = nl + 1
  }
  return str.length
}

/**
 * Maximal runs of LIVE `$` in `s`, as `{at, len}`. Run LENGTH is the unit that
 * matters: micromark's math-text closes a span only with a run of the SAME
 * length as its opener, so `$` and `$$` are different delimiters entirely.
 */
function dollarRuns(s: string): Array<{ at: number; len: number }> {
  const runs: Array<{ at: number; len: number }> = []
  let i = 0
  while (i < s.length) {
    if (s[i] !== '$') {
      i++
      continue
    }
    let slashes = 0
    while (i - slashes - 1 >= 0 && s[i - slashes - 1] === '\\') slashes++
    if (slashes % 2 === 1) {
      // `\$` — escaped, never a delimiter.
      i++
      continue
    }
    const at = i
    while (i < s.length && s[i] === '$') i++
    runs.push({ at, len: i - at })
  }
  return runs
}

/**
 * Pair the runs the way micromark does — left to right, each opener closed by
 * the next run of EQUAL length — and report the spans plus the runs that never
 * found a partner.
 */
function pairDollarRuns(runs: Array<{ at: number; len: number }>) {
  const spans: Array<{ start: number; end: number }> = []
  const unpaired: Array<{ at: number; len: number }> = []
  let i = 0
  while (i < runs.length) {
    let j = i + 1
    while (j < runs.length && runs[j].len !== runs[i].len) j++
    if (j < runs.length) {
      spans.push({ start: runs[i].at, end: runs[j].at + runs[j].len })
      i = j + 1
    } else {
      unpaired.push(runs[i])
      i++
    }
  }
  return { spans, unpaired }
}

/**
 * Would emitting a `$ … $` span at `[start, end)` collide with the `$` already in
 * this match's paragraph? This is the one way the rewrite can genuinely corrupt
 * output (see the module header), so it is the guard that decides.
 *
 * Two — and only two — situations are unsafe, both verified against the
 * installed micromark rather than assumed:
 *
 * 1. **The match sits inside an existing span.** `see $a \( b \) c$ end` — the
 *    injected delimiter lands in the middle of someone else's math and breaks it.
 *
 * 2. **An UNPAIRED single `$` is loose in the paragraph.** `cost $5 and \( E \)`
 *    → `cost $5 and $E$`, which micromark tokenizes as math `5 and ` plus a
 *    dangling literal `E$`. That is the hijack.
 *
 * Everything else is safe, and saying so is the point of pairing by run length
 * rather than counting dollars. In particular a `$$` run — paired or not —
 * CANNOT close the single `$` we emit, so a display block sitting in the same
 * paragraph no longer suppresses inline math. That matters because the display
 * pass deliberately emits `$$` with single newlines (so the block stays inside
 * its list item / blockquote), which leaves it in the SAME paragraph as any
 * following prose: `The energy is \[ E=mc^2 \] where \( m \) is mass.` used to
 * lose its `\( m \)` to this guard, and now keeps it.
 *
 * An already-PAIRED run of singles is likewise safe: pairing is left-to-right, so
 * runs we add after a resolved span cannot change how that span resolved.
 */
function paragraphBlocksInlineMath(str: string, start: number, end: number): boolean {
  const from = paragraphStart(str, start)
  const runs = dollarRuns(str.slice(from, paragraphEnd(str, end)))
  if (runs.length === 0) return false

  const { spans, unpaired } = pairDollarRuns(runs)
  const relStart = start - from
  const relEnd = end - from

  // (1) inside someone else's span — the match carries no `$` of its own, so it
  // is wholly inside or wholly outside; an overlap test settles it either way.
  if (spans.some(s => s.start < relEnd && s.end > relStart)) return true

  // (2) a lone `$` can pair with the delimiter we are about to inject. A loose
  // `$$`+ run cannot, so it does not block.
  return unpaired.some(r => r.len === 1)
}

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
 * remark-math parses.
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
function normalizeDisplayMath(md: string): string {
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

/**
 * Rewrite inline `\( … \)` into the `$ … $` form remark-math parses
 * (`singleDollarTextMath` is enabled — see `streamdownPlugins.ts`).
 *
 * Simpler than the display pass in one important way: `$…$` is a TEXT construct,
 * so it needs no newline injection and therefore none of the container /
 * blockquote / list-continuation machinery. It just has to survive seven guards,
 * every one of which degrades to "leave the text exactly as it renders today".
 *
 * MUST run after `normalizeDisplayMath` — see the module header.
 *
 * KNOWN LIMIT of the paragraph guard: `preprocessMarkdown` splits on code fences
 * and inline-code spans BEFORE calling us, so a paragraph interrupted by an
 * inline-code span arrives as two separate strings and a live `$` on the far side
 * of that span is invisible here. Deliberately not chased: excluding `$` that
 * lives INSIDE code is correct (it isn't live math), the residual case needs an
 * unpaired `$`, an inline-code span, and inline math in one paragraph, and the
 * result is the same mangling markdown already produces today for a lone `$`
 * beside any `$…$` span. The same part-local reasoning applies to `lineHead`
 * below, exactly as it already does in the display pass.
 */
function normalizeInlineMath(md: string): string {
  // One O(n) scan instead of a per-match paragraph scan: with no live `$` in the
  // string at all, no paragraph can contain one. This is the overwhelmingly
  // common case, and it keeps the guard off the hot path during streaming.
  const anyDollar = hasLiveDollar(md)

  return md.replace(
    INLINE_MATH_RE,
    (whole: string, body: string, offset: number, str: string) => {
      const inner = body.trim()
      if (!inner) return whole

      // A nested `\(` means the lazy closer matched the INNER `\)`, so converting
      // would emit a span plus a dangling literal `\)`. Same reasoning as the
      // display path's nested-`\[` guard.
      if (inner.includes('\\(')) return whole

      // A `$` anywhere in the body would close the emitted span early.
      if (inner.includes('$')) return whole

      // Regex, not math.
      if (BRE_SIGNAL_RE.test(inner)) return whole

      // An indented code block must never be touched. `preprocessMarkdown`
      // already splits out FENCED and INLINE code before calling us, so this
      // covers the one code form that split cannot see: the 4-column indented
      // block. `continuationPrefix` returns null for exactly that case.
      const lineHead = str.slice(str.lastIndexOf('\n', offset - 1) + 1, offset)
      if (continuationPrefix(lineHead) === null) return whole

      // Two ADJACENT pairs — `\( a \)\( b \)` with nothing between them — would
      // emit `$a$$b$`. A math-text closer must be a `$` run of the SAME length as
      // its opener, so the inner `$$` does not close the first span: the whole
      // thing parses as ONE span whose body is `a$$b`, which KaTeX then rejects.
      // Neither pair is safe to convert alone either (each would still abut the
      // other's literal `\(`/`\)`), so skip both. Rare in real output, and the
      // paragraph guard below cannot see it — that guard looks for a `$` already
      // in the source, whereas this collision is created by the rewrite itself.
      // Indexed rather than sliced on purpose: `str.slice(0, offset)` would copy
      // the whole prefix on every match, making the pass quadratic again.
      const after = offset + whole.length
      const abuts =
        str.startsWith('\\(', after) ||
        (offset >= 2 && str[offset - 1] === ')' && str[offset - 2] === '\\')
      if (abuts) return whole

      // The hijack guard — last because it is the only one that scans beyond the
      // match itself.
      if (anyDollar && paragraphBlocksInlineMath(str, offset, after)) return whole

      return `$${inner}$`
    },
  )
}

/**
 * Rewrite LaTeX's own math delimiters into the `$`-based forms remark-math
 * understands: display `\[ … \]` → `$$ … $$`, inline `\( … \)` → `$ … $`.
 *
 * Order is load-bearing: display first, inline second. The inline pass's
 * paragraph guard then sees the `$$` fences the display pass just emitted and
 * declines to rewrite a `\(` sitting inside a display body — which would produce
 * KaTeX-unparseable `$$\na $b$ c\n$$`.
 *
 * Both passes are idempotent (neither output contains `\[` or `\(`) and both are
 * safe on partial input: an unclosed opener simply fails to match, so running
 * this on every frame of a streaming response cannot corrupt a half-written
 * equation.
 */
export function normalizeMathDelimiters(md: string): string {
  if (typeof md !== 'string') return md

  const hasDisplay = md.indexOf('\\[') !== -1
  if (!hasDisplay && md.indexOf('\\(') === -1) return md

  const withDisplay = hasDisplay ? normalizeDisplayMath(md) : md
  // The display pass can introduce no `\(`, so this re-check is exact.
  return withDisplay.indexOf('\\(') === -1
    ? withDisplay
    : normalizeInlineMath(withDisplay)
}
