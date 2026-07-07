import { useCallback, useEffect, useRef, useState } from 'react'
import { collectMatches } from './matcher'
import { isHighlightSupported } from './highlightSupported'

// CSS Custom Highlight API registry names. Registered in a <style> by FindableRegion
// as ::highlight(file-find) / ::highlight(file-find-active).
const HL_ALL = 'file-find'
const HL_ACTIVE = 'file-find-active'

interface FindController {
  /** Total matches for the current query. */
  count: number
  /** 0-based index of the active match, or -1 when there are none. */
  activeIndex: number
  /** Advance to the next match (wraps). No-op when count is 0. */
  next: () => void
  /** Go to the previous match (wraps). No-op when count is 0. */
  prev: () => void
}

/** {node, offset} address inside the walked text. */
interface Addr {
  node: Text
  offset: number
}

/**
 * Find-in-document over a container, painting matches with the CSS Custom
 * Highlight API (no DOM mutation, so it survives shiki markup, Streamdown
 * re-renders, and `content-visibility` virtualization). Rebuilds on query change
 * and whenever the container subtree mutates while active (handles async render).
 */
export function useFindInDocument(
  containerRef: React.RefObject<HTMLElement | null>,
  query: string,
  active: boolean,
): FindController {
  const [count, setCount] = useState(0)
  const [activeIndex, setActiveIndex] = useState(-1)
  // Ordered match ranges, kept in a ref so next/prev + the active-highlight
  // effect can read them without re-running the (expensive) walk.
  const rangesRef = useRef<Range[]>([])

  const supported = isHighlightSupported()

  const clearHighlights = useCallback(() => {
    if (!supported) return
    CSS.highlights.delete(HL_ALL)
    CSS.highlights.delete(HL_ACTIVE)
    rangesRef.current = []
  }, [supported])

  // Walk text nodes → build Ranges for every match → register the "all" highlight.
  const rebuild = useCallback(() => {
    if (!supported) return
    const root = containerRef.current
    if (!root || !query) {
      clearHighlights()
      setCount(0)
      setActiveIndex(-1)
      return
    }
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
    const nodes: { node: Text; start: number }[] = []
    let text = ''
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const t = n as Text
      nodes.push({ node: t, start: text.length })
      text += t.data
    }
    // Map a global string offset onto a {textNode, localOffset}. Binary-searchable
    // but linear is fine (node counts are viewport-bounded via content-visibility).
    const addrOf = (offset: number): Addr | null => {
      for (let i = nodes.length - 1; i >= 0; i--) {
        const { node, start } = nodes[i]
        if (offset >= start) return { node, offset: offset - start }
      }
      return nodes.length ? { node: nodes[0].node, offset: 0 } : null
    }
    const spans = collectMatches(text, query)
    const ranges: Range[] = []
    for (const s of spans) {
      const a = addrOf(s.start)
      const b = addrOf(s.end)
      if (!a || !b) continue
      const r = document.createRange()
      try {
        r.setStart(a.node, a.offset)
        r.setEnd(b.node, b.offset)
      } catch {
        continue
      }
      ranges.push(r)
    }
    rangesRef.current = ranges
    if (ranges.length === 0) {
      clearHighlights()
      setCount(0)
      setActiveIndex(-1)
      return
    }
    // Highlight ctor accepts a spread of ranges.
    CSS.highlights.set(HL_ALL, new Highlight(...ranges))
    setCount(ranges.length)
    // Keep the active match if still valid; else start at the first.
    setActiveIndex(prev => (prev >= 0 && prev < ranges.length ? prev : 0))
  }, [supported, containerRef, query, clearHighlights])

  // (Re)build when the query / active flag changes, and observe the subtree so an
  // async-rendered body (Streamdown, shiki) re-matches once its DOM settles.
  useEffect(() => {
    if (!supported || !active) {
      clearHighlights()
      setCount(0)
      setActiveIndex(-1)
      return
    }
    rebuild()
    const root = containerRef.current
    if (!root || typeof MutationObserver === 'undefined') return
    let raf = 0
    const obs = new MutationObserver(() => {
      cancelAnimationFrame(raf)
      raf = requestAnimationFrame(rebuild)
    })
    obs.observe(root, { childList: true, subtree: true, characterData: true })
    return () => {
      cancelAnimationFrame(raf)
      obs.disconnect()
    }
  }, [supported, active, rebuild, clearHighlights, containerRef])

  // Clear everything on unmount so highlights don't leak to the next viewer.
  useEffect(() => clearHighlights, [clearHighlights])

  // Paint the active match distinctly + scroll it into view.
  useEffect(() => {
    if (!supported) return
    const ranges = rangesRef.current
    if (activeIndex < 0 || activeIndex >= ranges.length) {
      CSS.highlights.delete(HL_ACTIVE)
      return
    }
    const active = ranges[activeIndex]
    CSS.highlights.set(HL_ACTIVE, new Highlight(active))
    // Scroll the match's containing element into view. A content-visibility:auto
    // ancestor still exposes geometry (contain-intrinsic-size), so this positions
    // the scroller and forces the offscreen line to render.
    const el =
      active.startContainer.nodeType === Node.TEXT_NODE
        ? active.startContainer.parentElement
        : (active.startContainer as HTMLElement)
    el?.scrollIntoView({ block: 'center', inline: 'nearest' })
  }, [supported, activeIndex, count])

  const next = useCallback(() => {
    setActiveIndex(prev => {
      const n = rangesRef.current.length
      return n === 0 ? -1 : (prev + 1 + n) % n
    })
  }, [])
  const prev = useCallback(() => {
    setActiveIndex(p => {
      const n = rangesRef.current.length
      return n === 0 ? -1 : (p - 1 + n) % n
    })
  }, [])

  return { count, activeIndex, next, prev }
}
