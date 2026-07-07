// Pure global-offset → text-segment mapping for find-in-document. Extracted so
// the cross-segment (multi-text-node) address logic is unit-testable without a
// DOM (useFindInDocument walks Text nodes and feeds their start offsets here).

/**
 * Given the ascending start offsets of consecutive text segments and a global
 * character offset, return the index of the segment that CONTAINS the offset
 * (i.e. the last segment whose start ≤ offset). Returns 0 for an offset before
 * the first segment, and -1 when there are no segments.
 *
 * This is the primitive that lets a match spanning multiple text nodes resolve
 * its start node and end node independently.
 */
export function locateSegment(starts: number[], offset: number): number {
  if (starts.length === 0) return -1
  // Linear from the end (node counts are viewport-bounded via content-visibility);
  // the first segment whose start ≤ offset, scanning high→low.
  for (let i = starts.length - 1; i >= 0; i--) {
    if (offset >= starts[i]) return i
  }
  return 0
}
