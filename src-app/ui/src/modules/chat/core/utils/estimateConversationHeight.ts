import type { ConversationResponse } from '@/api-client/types'

/**
 * Content-aware first-pass row-height estimate for a virtualized
 * `ConversationCard` (chats-page-virtualization ITEM-1, DEC-1).
 *
 * Mirrors `estimateMessageHeight.ts`: the virtualizer measures a row's true
 * height only after it renders, so a fresh mount pays an estimate→measured
 * correction that moves the scroll geometry (the scrollbar-thumb jump / "jank").
 * A close first-pass estimate shrinks that correction toward zero.
 *
 * A conversation card is far more uniform than a chat message — its only real
 * height variable is whether the TITLE wraps to a second line (it is
 * `line-clamp-2`, so it never exceeds two). The meta row (message count +
 * relative time) sits INLINE with the title from the `sm` breakpoint up (the
 * virtualized/desktop path), so it does not add its own line; it only RESERVES
 * horizontal space, which makes a borderline title wrap sooner. We model exactly
 * that: `message_count > 0` widens the reserved meta, so the estimate is
 * monotonic non-decreasing in the meta width (never lower with a count than
 * without).
 *
 * Cheap (no DOM, no layout), TOTAL (undefined/empty title → floor), and memoized
 * per (conversation object, width bucket) in a WeakMap — parity with the message
 * estimator's hot-path guard.
 */

/** Card chrome: vertical padding + the `pb-6` bottom action-row reserve. */
const CARD_BASE = 56
/** Rendered title line height at `text-sm`. */
const TITLE_LINE_HEIGHT = 20
/** One-line card floor (also the undefined/empty-title return). */
export const FLOOR = CARD_BASE + TITLE_LINE_HEIGHT
/** Title is `line-clamp-2` — it never renders more than two lines. */
const MAX_TITLE_LINES = 2
/** px per title character at `text-sm` (~14px glyphs, ~7.5px advance). */
const PX_PER_CHAR = 7.5
/** Horizontal space the relative-time meta always reserves next to the title. */
const META_TIME_RESERVE = 88
/** Extra reserve when a message-count chip + separator is present. */
const META_COUNT_RESERVE = 84

/**
 * The estimator models the INLINE card layout (`ConversationCard` is
 * `flex-col sm:flex-row`, so the meta sits beside the title from the `sm` (640px)
 * VIEWPORT breakpoint up). This is the only layout the estimate ever needs: the
 * virtualized path runs ONLY on the desktop inner-scroll path (`virtualize =
 * !nativeScroll`), i.e. at a ≥ sm viewport where the card is always inline. A
 * genuinely narrow (< sm) VIEWPORT flips `nativeScroll` on → the plain,
 * non-virtualized render path, which never calls this estimator. (An earlier
 * attempt to branch on the passed CONTENT width was wrong — the card's `sm:` is a
 * VIEWPORT media query, not a content-width one, so a narrow content column at a
 * wide viewport still renders inline.)
 */
function charsPerLine(availWidth: number): number {
  return Math.max(12, Math.floor(availWidth / PX_PER_CHAR))
}

function computeEstimate(conv: ConversationResponse, width: number): number {
  const title = conv.title?.trim()
  if (!title) return FLOOR
  // The meta reserves horizontal space beside the title, so a borderline title
  // wraps sooner when a message-count chip is present.
  const metaReserve =
    META_TIME_RESERVE + (conv.message_count > 0 ? META_COUNT_RESERVE : 0)
  const availWidth = Math.max(80, width - metaReserve)
  const rawLines = Math.ceil(title.length / charsPerLine(availWidth))
  const lines = Math.min(MAX_TITLE_LINES, Math.max(1, rawLines))
  return CARD_BASE + lines * TITLE_LINE_HEIGHT
}

// Per-conversation memo (weak keys → no leak): a ConversationResponse object is
// immutable between store updates, so the estimate is stable per (conv, bucket).
const memo = new WeakMap<ConversationResponse, Map<number, number>>()

/**
 * Estimate the rendered CARD height (px) at the given content width — the card
 * body only; the virtualized Row wrapper adds its own `py-1.5` padding, which
 * `VirtualizedConversationList` adds to the size estimate at the measured-element
 * boundary. `width` is the scroll viewport's inner content width; the default
 * matches the app content column (`max-w-4xl` 896px − the 24px `px-3` row gutters
 * = 872), so a caller relying on the default buckets at the SAME width the row is
 * measured/seeded at.
 */
export function estimateConversationHeight(
  conv: ConversationResponse | undefined,
  width = 872,
): number {
  if (!conv) return FLOOR
  const bucket = Math.round(width / 120)
  let byBucket = memo.get(conv)
  if (byBucket) {
    const hit = byBucket.get(bucket)
    if (hit !== undefined) return hit
  } else {
    byBucket = new Map()
    memo.set(conv, byBucket)
  }
  const h = computeEstimate(conv, width)
  byBucket.set(bucket, h)
  return h
}
