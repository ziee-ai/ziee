import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { Button } from '@ziee/kit'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { cn } from '@/lib/utils'
import { COLLAPSE_MAX_HEIGHT_PX } from '@/modules/chat/components/collapsible'
import { Stores } from '@ziee/framework/stores'
import {
  useMessageViewStateStore,
  type MessageViewFullState,
} from '@/modules/chat/core/stores/MessageViewState.store'
import { DEFAULT_MESSAGE_COLLAPSED } from '@/modules/chat/core/stores/messageViewState.helpers'
import { useInPlaceAnchor } from '@/modules/chat/core/utils/useInPlaceAnchor'

/**
 * Horizontal + vertical inset applied to the clamped content box, in px. Must
 * stay in sync with the `-m-0.5 p-0.5` utility below (0.5 = 2px on Tailwind's
 * scale); it exists so the clamp height and the overflow probe can compensate
 * for the padding instead of silently losing 4px of the fold.
 */
const INSET_PX = 2

interface CollapsibleBlockProps {
  children: ReactNode
  /** Clamp height when collapsed (px). Defaults to `COLLAPSE_MAX_HEIGHT_PX`. */
  maxHeightPx?: number
  /** Stable message id. When present the collapsed flag is read from / written
   *  to the per-conversation `MessageViewState` store so it SURVIVES the
   *  virtualizer unmounting + remounting this row (message-scroll-stability
   *  ITEM-4). Absent → falls back to component-local state (uncontrolled). */
  messageId?: string
  className?: string
  'data-testid'?: string
}

/**
 * CollapsibleBlock (chat-power-features ITEM-3) — clamps tall content to
 * `maxHeightPx`, fading the bottom edge, and reveals a "Show more / Show less"
 * toggle. The toggle only appears when the content ACTUALLY overflows the clamp
 * (measured at runtime via a ResizeObserver), so content that fits renders
 * untouched. Collapsed by default.
 *
 * The collapsed flag is LIFTED into the `MessageViewState` store (keyed by
 * `messageId`) so expanding a long message persists across the virtualized
 * row's unmount/remount — scrolling away and back keeps it expanded
 * (message-scroll-stability ITEM-4). Toggling routes through `useInPlaceAnchor`
 * so the expand grows downward without the viewport jumping (ITEM-7).
 */
export function CollapsibleBlock({
  children,
  maxHeightPx = COLLAPSE_MAX_HEIGHT_PX,
  messageId,
  className,
  'data-testid': dataTestid = 'collapsible-block',
}: CollapsibleBlockProps) {
  const contentRef = useRef<HTMLDivElement>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const regionId = useId()
  const [overflowing, setOverflowing] = useState(false)

  // Collapsed source of truth: the lifted store when we have a message id
  // (survives remount), else local state (uncontrolled fallback for a
  // CollapsibleBlock used without an id). SCOPED selector — subscribe only to
  // THIS message's flag so toggling one message doesn't re-render every other
  // mounted collapsible.
  const storedCollapsed = useMessageViewStateStore((s: MessageViewFullState) =>
    messageId ? s.collapsed[messageId] : undefined,
  )
  const [localCollapsed, setLocalCollapsed] = useState(true)
  const collapsed = messageId
    ? storedCollapsed ?? DEFAULT_MESSAGE_COLLAPSED
    : localCollapsed

  const anchorBeforeChange = useInPlaceAnchor(rootRef)
  const setCollapsed = (next: boolean) => {
    anchorBeforeChange()
    if (messageId) Stores.MessageViewState.setMessageCollapsed(messageId, next)
    else setLocalCollapsed(next)
  }

  // The clamped element carries a 2px inset on every side (see the class list
  // below). Under `box-sizing: border-box` that padding would otherwise come out
  // of the clamp, so both the `max-height` and the overflow probe are widened by
  // it: the VISIBLE content area stays exactly `maxHeightPx`, the margin box
  // still occupies `maxHeightPx` (the equal negative margins cancel the growth),
  // and the probe keeps its original 1px tolerance instead of tripping 4px early.
  // Derived from one constant so the two can't drift apart.
  const clampBoxPx = maxHeightPx + INSET_PX * 2

  // Measure whether the full content is taller than the clamp. `scrollHeight`
  // reflects the full (unclamped) height even while a `max-height` is applied,
  // so this stays correct in the collapsed state. Re-measured on content/size
  // changes so a message that grows (or a late-loading inline preview) updates
  // the affordance.
  const measure = () => {
    const el = contentRef.current
    if (!el) return
    setOverflowing(el.scrollHeight > clampBoxPx + 1)
  }

  useLayoutEffect(() => {
    measure()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [children, maxHeightPx])

  useEffect(() => {
    const el = contentRef.current
    if (!el || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(() => measure())
    ro.observe(el)
    return () => ro.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [maxHeightPx])

  const isClamped = overflowing && collapsed

  return (
    <div ref={rootRef} className={cn('flex flex-col', className)} data-testid={dataTestid}>
      <div
        ref={contentRef}
        id={regionId}
        data-testid="collapsible-content"
        // If a focusable descendant (a link / copy button in a long answer)
        // receives focus while the block is clamped, auto-expand so its focus
        // ring isn't clipped or alpha-faded below the fold (WCAG 2.4.7/2.4.11).
        onFocusCapture={e => {
          if (isClamped && e.target !== e.currentTarget) setCollapsed(false)
        }}
        className={cn(
          'relative',
          // -m-0.5 p-0.5: a 2px inset on ALL FOUR sides. Equal negative margin +
          // padding cancel, so content is laid out in the SAME WIDTH as before
          // while this element's box edge moves outward.
          //
          // Why it is needed: when clamped, this element gets BOTH
          // `overflow-hidden` and `mask-image` below, and each clips to the
          // border box — `mask-clip` defaults to `border-box`, so the mask's
          // painting area excludes anything drawn OUTSIDE the box exactly as the
          // overflow clip does. A child kit Card's border is `ring-1`: a
          // box-shadow with 1px spread and no offset, painted entirely outside
          // its own box. Sitting flush against this container, all of it landed
          // in the clipped zone and vanished while collapsed — the Thinking and
          // tool-call boxes rendered with no border and read as washed out
          // (issue #183). Removing EITHER clip alone did not restore it; the
          // inset gives the ring room inside both.
          //
          // All four sides, not just horizontal: the FIRST child card's top edge
          // is flush with this container's top, so a horizontal-only inset left
          // its top hairline clipped (measured: top ring delta 0 collapsed vs 25
          // expanded). This is the same failure the sibling comment in
          // ChatMessage describes for `overflow-x: hidden`.
          //
          // Vertically the 2px overhang is safe: the ancestor deliberately uses
          // `overflow-x: clip` (NOT hidden) so `overflow-y` stays truly visible,
          // and nothing above clips on the y axis. Horizontally the 4px of growth
          // is absorbed by the bubble's own `px-0.5`.
          //
          // Applied unconditionally rather than gated on isClamped simply because
          // it is self-cancelling and therefore inert when unclamped — one fewer
          // state-dependent style to reason about. (It would NOT reflow either
          // way: the margin and padding offset, so the content width is the same
          // with or without the classes.)
          //
          // `max-height` bounds the BORDER box under `box-sizing: border-box`,
          // so the vertical padding would come out of the visible area. Both the
          // clamp and the overflow probe are widened by `INSET_PX * 2` to
          // compensate (see `clampBoxPx`), which keeps the visible content at
          // exactly `maxHeightPx`, keeps the MARGIN box at `maxHeightPx` (the
          // negative margins cancel the growth, so surrounding layout is
          // unchanged), and leaves the probe's 1px tolerance intact.
          //
          // The horizontal half REQUIRES a parent with >= 2px horizontal padding
          // (today: ChatMessage's `px-0.5`) to absorb the overhang. That
          // cross-file invariant IS covered: the regression spec measures each
          // card against its tightest clipping ancestor on each axis, so removing
          // that padding fails TEST-3 with "only 0px … on LEFT" (verified).
          '-m-0.5 p-0.5',
          // Bottom fade cueing there's more below. A mask (not a color overlay)
          // fades the content itself to transparent, so it blends over ANY
          // background (the primary/10 user bubble as well as the transparent
          // assistant body) — no mismatched color band.
          isClamped &&
            'overflow-hidden [mask-image:linear-gradient(to_bottom,black_75%,transparent)]',
        )}
        style={isClamped ? { maxHeight: clampBoxPx } : undefined}
        data-collapsed={overflowing ? collapsed : undefined}
      >
        {children}
      </div>
      {overflowing && (
        <Button
          data-testid="collapsible-toggle"
          variant="ghost"
          // mt-1.5, not mt-1: the content div above carries `-m-0.5`, whose -2px
          // bottom margin would otherwise pull this up to a 2px gap and off the
          // 4px spacing rhythm. 6px - 2px restores the original 4px.
          className="mt-1.5 self-start h-auto px-2 py-1 text-xs text-muted-foreground"
          icon={collapsed ? <ChevronDown /> : <ChevronUp />}
          onClick={() => setCollapsed(!collapsed)}
          aria-expanded={!collapsed}
          aria-controls={regionId}
        >
          {collapsed ? 'Show more' : 'Show less'}
        </Button>
      )}
    </div>
  )
}
