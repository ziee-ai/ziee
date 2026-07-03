import { useEffect, useRef, useState } from 'react'

/**
 * Document-scroll proof (public, /scroll-proof).
 *
 * Confirmed: with document scroll + viewport-fit=cover, content already extends
 * under the notch / home indicator. The only reason the rows weren't visible up
 * there is the opaque header covering that area. This demonstrates the two
 * native patterns:
 *  - translucent header whose background fills the safe area (content shows
 *    through, blurred, as it scrolls under), and
 *  - hide-on-scroll: the header slides away scrolling down (content fully under
 *    the notch), returns scrolling up — like Safari's own toolbar.
 * Toggle between them to compare.
 */
export default function ScrollProofPage() {
  const rows = Array.from({ length: 60 }, (_, i) => i + 1)
  const [hideOnScroll, setHideOnScroll] = useState(true)
  const [hidden, setHidden] = useState(false)
  const lastY = useRef(0)

  useEffect(() => {
    const onScroll = () => {
      const y = window.scrollY
      if (hideOnScroll) {
        // hide when scrolling down past a threshold, show when scrolling up
        if (y > lastY.current && y > 60) setHidden(true)
        else if (y < lastY.current) setHidden(false)
      } else {
        setHidden(false)
      }
      lastY.current = y
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [hideOnScroll])

  return (
    <div className="min-h-dvh w-full bg-background text-foreground">
      {/* Translucent header. padding-top = safe-area inset so its background
          fills the notch strip; content scrolls under it (blurred). Slides up
          out of view when `hidden` (hide-on-scroll mode). */}
      <header
        className="fixed inset-x-0 top-0 z-20 border-b border-border bg-card/75 backdrop-blur-md px-4 pb-3 transition-transform duration-300"
        style={{
          paddingTop: 'calc(env(safe-area-inset-top, 0px) + 12px)',
          transform: hidden ? 'translateY(-100%)' : 'translateY(0)',
        }}
      >
        <h1 className="text-base font-semibold">Scroll proof</h1>
        <p className="text-xs text-muted-foreground">
          Content flows under this header (and the notch). Scroll to feel it.
        </p>
        <button
          className="mt-2 rounded-md border border-border px-3 py-1 text-xs"
          onClick={() => setHideOnScroll((v) => !v)}
        >
          Header mode: {hideOnScroll ? 'hide-on-scroll' : 'always translucent'} (tap to toggle)
        </button>
      </header>

      {/* Spacer so the first rows aren't hidden behind the fixed header at rest.
          ~safe-area + header height. */}
      <div style={{ height: 'calc(env(safe-area-inset-top, 0px) + 116px)' }} />

      <main className="mx-auto w-full max-w-md px-4 pb-4 flex flex-col gap-3">
        {rows.map((n) => (
          <div key={n} className="rounded-lg border border-border bg-card px-4 py-6 text-sm">
            Row {n} of {rows.length}
          </div>
        ))}
      </main>

      <footer
        className="px-4 pt-8 text-center text-xs text-muted-foreground"
        style={{ paddingBottom: 'calc(env(safe-area-inset-bottom, 0px) + 32px)' }}
      >
        End of list.
      </footer>
    </div>
  )
}
