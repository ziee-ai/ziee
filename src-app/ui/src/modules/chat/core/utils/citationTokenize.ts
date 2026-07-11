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
const CITE_RE = /\[(\d{1,3})\](?!\()/g

export function citationTokenize(text: string): string {
  return text.replace(CITE_RE, (_m, n: string) => `[${n}](#kb-cite-${n})`)
}

/** The href a tokenized citation chip carries (used by the `a` override). */
export function isCitationHref(href: string | undefined): number | null {
  if (!href) return null
  const m = /^#kb-cite-(\d{1,3})$/.exec(href)
  return m ? Number(m[1]) : null
}
