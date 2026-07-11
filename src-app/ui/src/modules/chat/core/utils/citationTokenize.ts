/**
 * Turn bare `[n]` citation markers the model emits (when grounding on knowledge
 * bases) into hash links `[n](#kb-cite-n)` so the chat markdown `a` override can
 * render them as focusable inline citation CHIPS that jump to the retrieval
 * transparency panel's n-th passage.
 *
 * Deliberately conservative — it must NOT touch:
 *  - real links `[text](url)`      (negative lookahead on `(`)
 *  - footnote refs `[^1]`          (the `^` means `\[(\d` never matches)
 *  - non-numeric brackets `[TODO]` (only 1–3 digit runs)
 *  - already-tokenized `[1](#kb-cite-1)` (the lookahead again)
 *
 * Pure + unit-tested; applied only to ASSISTANT text.
 */
// Match a bare `[n]` used as a citation: NOT preceded by a word char or `]`
// (so an array index `arr[1]` and a reference usage `[Smith][1]` are left
// alone — a citation reads "claim [1]" with a break before it), and NOT
// followed by `(` (a real link) or `:` (a `[1]: url` reference-definition).
const CITE_RE = /(?<![\w\]])\[(\d{1,3})\](?![(:])/g

/** Split on fenced code blocks (```…```) and inline code spans (`…`), so we
 *  never rewrite `[n]` that appears INSIDE code (which streamdown renders
 *  verbatim — a rewrite there would corrupt the displayed code). */
const CODE_SEGMENT_RE = /(```[\s\S]*?```|`[^`]*`)/g

export function citationTokenize(text: string): string {
  return text
    .split(CODE_SEGMENT_RE)
    .map(seg =>
      // Odd segments (the captured code spans) pass through untouched.
      seg.startsWith('`')
        ? seg
        : seg.replace(CITE_RE, (_m, n: string) => `[${n}](#kb-cite-${n})`),
    )
    .join('')
}

/** The href a tokenized citation chip carries (used by the `a` override). */
export function isCitationHref(href: string | undefined): number | null {
  if (!href) return null
  const m = /^#kb-cite-(\d{1,3})$/.exec(href)
  return m ? Number(m[1]) : null
}
