/**
 * Pure helpers for the windowed (chunk-on-demand) raw-code view.
 *
 * The TEXT/CODE viewer keeps EVERY line's text node in the DOM (so
 * find-in-document's TreeWalker spans the whole file — see `useFindInDocument`),
 * but defers the expensive Shiki HIGHLIGHT until a chunk scrolls into view,
 * mirroring the PDF viewer's page-on-demand model (`pdf/body.tsx`). These
 * functions carry the chunking + cap + reserved-height + escape logic so they are
 * unit-testable without a DOM (the suite runs under `node:test`, no jsdom).
 */

/** Lines per windowed chunk. Small enough that one chunk's Shiki pass is cheap
 *  (<~10 ms), large enough to keep the chunk/observer count modest even at the
 *  line cap (300k / 500 = 600 chunks). */
export const RAWCODE_CHUNK_LINES = 500

/** Raised OOM-backstop line cap (was 10k). Below it the FULL file renders
 *  (windowed); at/above it the file is truncated to the cap with a banner. This
 *  guards ONLY the pathological "many-bytes-of-newlines → millions of DOM rows"
 *  case that the upstream 10 MB byte cap cannot bound (10 MB of `\n` is ~10M
 *  lines). Real files sit far below it and never truncate. */
export const RAWCODE_MAX_LINES = 300_000

/** Per-line reserved height (px) used for a chunk's `contain-intrinsic-size`, so
 *  the scrollbar geometry is accurate without laying out offscreen chunks.
 *  No-wrap ≈ line-height 1.55 × 13px font ≈ 20.15 → 22. Wrap mode uses a larger
 *  estimate because a wrapped long line occupies multiple visual rows. */
export const LINE_PX = 22
export const LINE_PX_WRAP = 44

export interface LineChunk {
  /** 0-based global index of this chunk's first line (line-number offset). */
  startLine: number
  /** The raw source lines in this chunk. */
  lines: string[]
  /** The chunk's lines re-joined with `\n` (byte-exact slice of the source). */
  text: string
}

/**
 * Split an array of source lines into contiguous fixed-size chunks, each tagged
 * with the 0-based global index of its first line (so line numbers stay
 * continuous across chunks). `size` must be ≥ 1.
 */
export function chunkLineArray(lines: string[], size: number): LineChunk[] {
  const step = Math.max(1, Math.floor(size))
  const chunks: LineChunk[] = []
  // A zero-length source still has one (empty) line, matching `''.split('\n')`.
  const src = lines.length === 0 ? [''] : lines
  for (let i = 0; i < src.length; i += step) {
    const slice = src.slice(i, i + step)
    chunks.push({ startLine: i, lines: slice, text: slice.join('\n') })
  }
  return chunks
}

/**
 * Split a source string into fixed-size line-chunks. `chunks.map(c => c.text)
 * .join('\n')` byte-exactly reconstructs the input.
 */
export function chunkLines(text: string, size: number): LineChunk[] {
  return chunkLineArray(text.split('\n'), size)
}

/**
 * Apply the OOM-backstop line cap. Below the cap the lines pass through
 * (`truncated:false`); at/above it they are sliced to exactly `cap` lines
 * (`truncated:true`).
 */
export function applyLineCap(
  lines: string[],
  cap: number,
): { lines: string[]; truncated: boolean } {
  if (lines.length > cap) return { lines: lines.slice(0, cap), truncated: true }
  return { lines, truncated: false }
}

/**
 * Reserved intrinsic height (px) for a chunk of `lineCount` lines, used as the
 * `contain-intrinsic-size` so offscreen chunks reserve accurate scroll geometry.
 * Wrap mode reserves more per line (wrapped lines are taller). Scales linearly
 * with the line count.
 */
export function chunkReservedHeight(lineCount: number, wrap: boolean): number {
  return Math.max(0, lineCount) * (wrap ? LINE_PX_WRAP : LINE_PX)
}

/** HTML-escape a source string for injection into a plain (un-highlighted) chunk.
 *  `unescape` in a browser round-trips this back to the original text, which is
 *  the invariant find-in-document relies on when a chunk swaps plain→highlighted
 *  (the visible text — hence the searchable text — is identical either way). */
export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

/**
 * Build the inner `.line-code` HTML for ONE plain (un-highlighted) source line —
 * just the escaped text. A highlighted line wraps the same text in colored token
 * spans; both have identical `textContent`, so find Ranges stay valid across the
 * plain→highlight swap.
 */
export function plainLineCodeInner(line: string): string {
  return escapeHtml(line)
}

/**
 * Build the full plain (un-highlighted) `<pre class="shiki">` HTML for a chunk,
 * structurally identical to Shiki's transformed output (`.line` grid rows, each
 * with a sticky `.line-number` gutter + a `.line-code` wrapper), so upgrading a
 * chunk to highlighted causes NO layout shift. Line numbers are global
 * (1-based), offset by the chunk's `startLine`. The `\n` between lines matches
 * Shiki's own inter-line text nodes so a chunk's `textContent` (and thus the
 * find text) is identical whether plain or highlighted.
 */
export function buildPlainChunkHtml(chunk: LineChunk): string {
  const rows = chunk.lines
    .map(
      (line, i) =>
        `<span class="line"><span class="line-number">${
          chunk.startLine + i + 1
        }</span><span class="line-code">${plainLineCodeInner(line)}</span></span>`,
    )
    .join('\n')
  return `<pre class="shiki" tabindex="0"><code>${rows}</code></pre>`
}
