import type {
  MessageWithContent,
  MessageContent,
  MessageContentDataText,
} from '@/api-client/types'

/**
 * Content-aware first-pass row-height estimate for the virtualized message list
 * (message-scroll-perf ITEM-1, DEC-1).
 *
 * The virtualizer's old `estimateSize: () => 140` constant was 5–10× off for
 * tables / images / long answers, so every unmeasured row that scrolled into
 * view corrected `getTotalSize()` and made the scrollbar thumb jump. This
 * estimator inspects a message's content blocks and returns a height within
 * ~1.5× of reality, so the estimate→measured correction (and the thumb jump)
 * shrinks toward zero. It is intentionally cheap (no DOM, no layout) and TOTAL:
 * an undefined/empty message returns the floor, so it never throws where the
 * old constant could not.
 */

/** Bubble chrome: vertical padding + the actions/branch-navigator row. */
const BASE = 72
/** Floor — the short-user-turn estimate; matches the historical constant. */
export const FLOOR = 140
/** Per-text-block additive cap so one huge answer can't dominate the estimate. */
const TEXT_BLOCK_CAP = 900
const LINE_HEIGHT = 24
/** Flat adds for heavy block kinds (tuned to their rendered/ capped heights). */
const TABLE_ADD = 300 // near the min(60vh,36rem) MarkdownTable cap midpoint
const IMAGE_ADD = 240 // matches ReservedImage's dimensionless reserve (DEC-4)
const CODE_ADD = 160
const TOOL_ADD = 120 // tool_use / tool_result / file_attachment / image(file)

/** Chars per rendered line at a given content width (≈8px/char, min 24). */
function charsPerLine(width: number): number {
  return Math.max(24, Math.floor(width / 8))
}

/** Height a single text block's prose contributes at the given width. */
function textBlockHeight(text: string, width: number): number {
  if (!text) return 0
  const lines = Math.ceil(text.length / charsPerLine(width))
  return Math.min(TEXT_BLOCK_CAP, lines * LINE_HEIGHT)
}

function hasMarkdownTable(text: string): boolean {
  // A GFM table always has a delimiter row containing a pipe + dashes.
  return /\|\s*:?-{3,}/.test(text) || /\|-{3,}/.test(text)
}

function hasMarkdownImage(text: string): boolean {
  return text.includes('![') || /<img[\s>]/i.test(text)
}

function hasCodeFence(text: string): boolean {
  return text.includes('```')
}

/** Additive height contribution of one content block. */
function blockHeight(block: MessageContent, width: number): number {
  switch (block.content_type) {
    case 'text':
    case 'thinking': {
      const text = (block.content as MessageContentDataText)?.text ?? ''
      let h = textBlockHeight(text, width)
      if (hasMarkdownTable(text)) h += TABLE_ADD
      if (hasMarkdownImage(text)) h += IMAGE_ADD
      if (hasCodeFence(text)) h += CODE_ADD
      return h
    }
    case 'image':
      return IMAGE_ADD
    case 'tool_use':
    case 'tool_result':
    case 'file_attachment':
      return TOOL_ADD
    default:
      // Unknown / small block kinds (elicitation, errors) — a modest constant.
      return LINE_HEIGHT * 2
  }
}

/**
 * Estimate the rendered height (px) of a message row at the given content width.
 * `width` should be the scroll viewport's inner content width; callers pass the
 * app's `max-w-4xl` fallback (768) before the scroller is ready (DEC-1).
 */
export function estimateMessageHeight(
  message: MessageWithContent | undefined,
  width = 768,
): number {
  const contents = message?.contents
  if (!contents || contents.length === 0) return FLOOR
  let sum = BASE
  for (const block of contents) sum += blockHeight(block, width)
  return Math.max(FLOOR, Math.round(sum))
}
