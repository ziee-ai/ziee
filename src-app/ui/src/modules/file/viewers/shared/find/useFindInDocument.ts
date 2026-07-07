import { useCallback, useEffect, useRef, useState } from 'react'
import { collectMatches } from './matcher'
import { locateSegment } from './offset'
import { isHighlightSupported } from './highlightSupported'

// CSS Custom Highlight API registry names. Registered in a <style> by FindableRegion
// as ::highlight(file-find) / ::highlight(file-find-active). The registry is
// process-global, so FindableRegion namespaces the registered names per instance
// and passes them in (two open viewers must not clobber each other).
export interface HighlightNames {
  all: string
  active: string
}

interface FindController {
  count: number
  activeIndex: number
  next: () => void
  prev: () => void
}

interface Addr {
  node: Text
  offset: number
}

/** Debounce (ms) before re-walking on a query change — one keystroke shouldn't
 *  trigger a full O(N) tree walk mid-word. */
const REBUILD_DEBOUNCE_MS = 100

/**
 * Find-in-document over a container, painting matches with the CSS Custom
 * Highlight API (no DOM mutation, so it survives shiki markup, Streamdown
 * re-renders, and `content-visibility` virtualization). Rebuilds (debounced) on
 * query change and whenever the container subtree mutates while active.
 */
export function useFindInDocument(
  containerRef: React.RefObject<HTMLElement | null>,
  query: string,
  active: boolean,
  names: HighlightNames,
): FindController {
  const [count, setCount] = useState(0)
  const [activeIndex, setActiveIndex] = useState(-1)
  const rangesRef = useRef<Range[]>([])
  // Latest activeIndex, readable inside rebuild without making rebuild depend on
  // it (which would thrash the observer). Kept in sync below.
  const activeIndexRef = useRef(-1)
  activeIndexRef.current = activeIndex

  const supported = isHighlightSupported()

  const clearHighlights = useCallback(() => {
    if (!supported) return
    CSS.highlights.delete(names.all)
    CSS.highlights.delete(names.active)
    rangesRef.current = []
  }, [supported, names.all, names.active])

  // Paint the active match distinctly (used by rebuild + the index effect so the
  // active highlight is refreshed even when a rebuild keeps count/index the same).
  const paintActive = useCallback(
    (index: number) => {
      if (!supported) return
      const ranges = rangesRef.current
      if (index < 0 || index >= ranges.length) {
        CSS.highlights.delete(names.active)
        return
      }
      CSS.highlights.set(names.active, new Highlight(ranges[index]))
    },
    [supported, names.active],
  )

  // Walk text nodes → build Ranges for every match → register the highlights.
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
    const nodes: Text[] = []
    const starts: number[] = []
    let text = ''
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const t = n as Text
      nodes.push(t)
      starts.push(text.length)
      text += t.data
    }
    const addrOf = (offset: number): Addr | null => {
      const i = locateSegment(starts, offset)
      if (i < 0) return null
      return { node: nodes[i], offset: offset - starts[i] }
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
    CSS.highlights.set(names.all, new Highlight(...ranges))
    setCount(ranges.length)
    // Keep the active match if still valid; else start at the first.
    const nextActive =
      activeIndexRef.current >= 0 && activeIndexRef.current < ranges.length
        ? activeIndexRef.current
        : 0
    setActiveIndex(nextActive)
    // Refresh the active highlight against the FRESH range objects immediately —
    // the index effect won't re-run when count/index are unchanged.
    paintActive(nextActive)
  }, [supported, containerRef, query, clearHighlights, paintActive, names.all])

  // Keep a stable ref to the latest rebuild so the observer effect below doesn't
  // depend on `query` (which would disconnect+reconnect the observer per keystroke).
  const rebuildRef = useRef(rebuild)
  rebuildRef.current = rebuild

  // (Re)build on query/active change — DEBOUNCED so a fast typist doesn't trigger
  // a full walk on every keystroke.
  useEffect(() => {
    if (!supported || !active) {
      clearHighlights()
      setCount(0)
      setActiveIndex(-1)
      return
    }
    const id = setTimeout(() => rebuildRef.current(), REBUILD_DEBOUNCE_MS)
    return () => clearTimeout(id)
  }, [supported, active, query, clearHighlights])

  // Observe the subtree so an async-rendered body (Streamdown, shiki) re-matches
  // once its DOM settles. Stable deps (no `query`) → set up once per active session.
  useEffect(() => {
    if (!supported || !active) return
    const root = containerRef.current
    if (!root || typeof MutationObserver === 'undefined') return
    let raf = 0
    const obs = new MutationObserver(() => {
      cancelAnimationFrame(raf)
      raf = requestAnimationFrame(() => rebuildRef.current())
    })
    obs.observe(root, { childList: true, subtree: true, characterData: true })
    return () => {
      cancelAnimationFrame(raf)
      obs.disconnect()
    }
  }, [supported, active, containerRef])

  // Clear everything on unmount so highlights don't leak to the next viewer.
  useEffect(() => clearHighlights, [clearHighlights])

  // Repaint + scroll when the active index changes (next/prev).
  useEffect(() => {
    if (!supported) return
    const ranges = rangesRef.current
    if (activeIndex < 0 || activeIndex >= ranges.length) {
      CSS.highlights.delete(names.active)
      return
    }
    paintActive(activeIndex)
    const r = ranges[activeIndex]
    const el =
      r.startContainer.nodeType === Node.TEXT_NODE
        ? r.startContainer.parentElement
        : (r.startContainer as HTMLElement)
    el?.scrollIntoView({ block: 'center', inline: 'nearest' })
  }, [supported, activeIndex, count, paintActive, names.active])

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
