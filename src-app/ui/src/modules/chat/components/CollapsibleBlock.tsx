import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { Button } from '@/components/ui'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { cn } from '@/lib/utils'
import { COLLAPSE_MAX_HEIGHT_PX } from '@/modules/chat/components/collapsible'

interface CollapsibleBlockProps {
  children: ReactNode
  /** Clamp height when collapsed (px). Defaults to `COLLAPSE_MAX_HEIGHT_PX`. */
  maxHeightPx?: number
  className?: string
  'data-testid'?: string
}

/**
 * CollapsibleBlock (ITEM-3) — clamps tall content to `maxHeightPx`, fading the
 * bottom edge, and reveals a "Show more / Show less" toggle. The toggle only
 * appears when the content ACTUALLY overflows the clamp (measured at runtime via
 * a ResizeObserver), so content that fits renders untouched. Collapsed by
 * default.
 */
export function CollapsibleBlock({
  children,
  maxHeightPx = COLLAPSE_MAX_HEIGHT_PX,
  className,
  'data-testid': dataTestid = 'collapsible-block',
}: CollapsibleBlockProps) {
  const contentRef = useRef<HTMLDivElement>(null)
  const [collapsed, setCollapsed] = useState(true)
  const [overflowing, setOverflowing] = useState(false)

  // Measure whether the full content is taller than the clamp. `scrollHeight`
  // reflects the full (unclamped) height even while a `max-height` is applied,
  // so this stays correct in the collapsed state. Re-measured on content/size
  // changes so a message that grows (or a late-loading inline preview) updates
  // the affordance.
  const measure = () => {
    const el = contentRef.current
    if (!el) return
    setOverflowing(el.scrollHeight > maxHeightPx + 1)
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
    <div className={cn('flex flex-col', className)} data-testid={dataTestid}>
      <div
        ref={contentRef}
        className={cn('relative', isClamped && 'overflow-hidden')}
        style={isClamped ? { maxHeight: maxHeightPx } : undefined}
        data-collapsed={overflowing ? collapsed : undefined}
      >
        {children}
        {isClamped && (
          // Bottom fade cueing there's more below. Theme-aware token gradient;
          // pointer-events-none so it never blocks selection/clicks.
          <div className="pointer-events-none absolute inset-x-0 bottom-0 h-12 bg-gradient-to-b from-transparent to-background" />
        )}
      </div>
      {overflowing && (
        <Button
          data-testid="collapsible-toggle"
          variant="ghost"
          className="mt-1 self-start h-auto px-2 py-1 text-xs text-muted-foreground"
          icon={collapsed ? <ChevronDown /> : <ChevronUp />}
          onClick={() => setCollapsed(c => !c)}
          aria-expanded={!collapsed}
        >
          {collapsed ? 'Show more' : 'Show less'}
        </Button>
      )}
    </div>
  )
}
