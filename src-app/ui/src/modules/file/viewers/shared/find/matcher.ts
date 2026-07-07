// Pure substring matcher for find-in-document. Extracted so the match semantics
// (case-insensitive, non-overlapping, ordered) are unit-testable without a DOM;
// the DOM walker in useFindInDocument maps these string offsets onto Ranges.

export interface MatchSpan {
  /** Inclusive start offset into the haystack. */
  start: number
  /** Exclusive end offset into the haystack. */
  end: number
}

/**
 * Collect every case-insensitive, NON-overlapping occurrence of `needle` in
 * `haystack`, in ascending order. An empty or whitespace-only needle yields no
 * matches (so an empty query never "matches everything").
 *
 * Non-overlapping: after a hit at [i, i+len) the scan resumes at i+len, so
 * `collectMatches('aaaa', 'aa')` → 2 matches, not 3.
 */
export function collectMatches(haystack: string, needle: string): MatchSpan[] {
  const spans: MatchSpan[] = []
  if (!needle || needle.trim() === '' || !haystack) return spans
  const hay = haystack.toLowerCase()
  const need = needle.toLowerCase()
  const len = need.length
  let from = 0
  for (;;) {
    const i = hay.indexOf(need, from)
    if (i === -1) break
    spans.push({ start: i, end: i + len })
    from = i + len
  }
  return spans
}
