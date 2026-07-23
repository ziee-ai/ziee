// Conversation titles and list labels are PLAIN STRINGS, not markdown. They are
// rendered into a sidebar row, a header, a tooltip, an `aria-label`, and a search
// filter — so they never reach Streamdown and a `\( m \)` in them surfaces to the
// user verbatim, which is what `normalizeMathDelimiters` fixed everywhere else:
//
//     Check: the energy is \[ E = mc^2 \] where \( m \) ...
//
// Rendering real KaTeX here is the wrong tool. The label's type is `string` and
// several of its consumers (aria-label, tooltip, the search predicate, and any
// future `document.title`) cannot hold a React node at all; turning it into one
// would also put a display-math block inside a 32px sidebar row. What the label
// needs is the plain-text READING of the math, which is what this does.
//
// The rule mirrors what the message body now shows, so the two agree: a balanced
// pair is math, and its rendering is its CONTENT (the delimiters are syntax, not
// characters) — `\( m \)` reads as `m`, exactly as the body renders it. Anything
// left over is not a delimiter pair but an escaped punctuation character, so it
// unescapes to that character, which is what markdown's own character-escape rule
// does with it in prose.
//
// Known limit: a LaTeX command inside the pair stays literal — `\( \lambda \)`
// reads as `\lambda`, not `λ`. Mapping commands to unicode needs a real TeX
// table, and a partial one is worse than none (a title that silently drops the
// symbols it can't map is harder to read than one that shows them). The common
// cases models actually produce inline — a bare symbol, a short relation like
// `E = mc^2` — read correctly.

// Same shapes (and the same doubly-escaped-opener lookbehind + length caps) as
// `normalizeMathDelimiters`, so the two passes agree on what a delimiter pair is.
const DISPLAY_PAIR_RE = /(?<!\\)\\\[([\s\S]{0,2000}?)\\\]/g
const INLINE_PAIR_RE = /(?<!\\)\\\(([^\r\n]{0,300}?)\\\)/g

/** A leftover escaped bracket/paren — not part of a pair, so just punctuation. */
const ESCAPED_PUNCT_RE = /(?<!\\)\\([[\]()])/g

/**
 * The plain-text reading of a string that may carry LaTeX math delimiters, for
 * display surfaces that cannot render markdown (titles, list labels, tooltips).
 *
 * Whitespace is collapsed to single spaces because a label is always one line —
 * a `first_message_preview` can carry the newlines of the original message, and
 * unwrapping a display block leaves its padding behind.
 */
export function mathToPlainText(text: string): string {
  if (typeof text !== 'string') return text
  // No backslash means no delimiter and no escape, so nothing here can apply.
  // Keeps the overwhelmingly common label byte-for-byte identical.
  if (text.indexOf('\\') === -1) return text

  return text
    .replace(DISPLAY_PAIR_RE, (_whole, inner: string) => inner.trim())
    .replace(INLINE_PAIR_RE, (_whole, inner: string) => inner.trim())
    .replace(ESCAPED_PUNCT_RE, '$1')
    .replace(/\s+/g, ' ')
    .trim()
}
