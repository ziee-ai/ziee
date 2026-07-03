/**
 * Document-scroll + safe-area diagnostic (public, /scroll-proof).
 *
 * Tall column in normal document flow (body/window is the scroller) → iOS Safari
 * collapses its toolbar and flows content under it. The bright fixed strips + the
 * printed env(safe-area-inset-*) values show whether content reaches under the
 * notch / home indicator (needs viewport-fit=cover, which is set in index.html).
 */
export default function ScrollProofPage() {
  const rows = Array.from({ length: 60 }, (_, i) => i + 1)
  return (
    <div className="min-h-dvh w-full bg-background text-foreground">
      {/* FULL-BLEED fixed strips at the true top/bottom edges — extend INTO the
          safe areas via negative env() margins so you can see if pixels reach
          under the notch / home indicator. Bright colors = unmissable. */}
      <div
        data-allow-custom-color
        className="fixed inset-x-0 top-0 z-50 text-center text-[10px] font-bold text-white"
        style={{
          height: 'calc(env(safe-area-inset-top, 0px) + 24px)',
          background: 'magenta',
        }}
      >
        TOP EDGE (magenta should reach the notch)
      </div>
      <div
        data-allow-custom-color
        className="fixed inset-x-0 bottom-0 z-50 text-center text-[10px] font-bold text-white"
        style={{
          height: 'calc(env(safe-area-inset-bottom, 0px) + 24px)',
          background: 'lime',
          color: 'black',
        }}
      >
        BOTTOM EDGE (lime should reach the home indicator)
      </div>

      <header className="sticky top-0 z-10 bg-card/90 backdrop-blur border-b border-border px-4 pt-12 pb-3">
        <h1 className="text-base font-semibold">Scroll + safe-area proof</h1>
        <p className="text-xs text-muted-foreground">
          Read the inset values below and tell me if the magenta/lime strips
          reach under the notch / home indicator.
        </p>
        <pre
          className="mt-2 text-[11px] leading-tight text-foreground"
          id="safe-area-readout"
          ref={(el) => {
            if (!el) return
            const probe = document.createElement('div')
            probe.style.cssText =
              'position:fixed;top:env(safe-area-inset-top);left:env(safe-area-inset-left);right:env(safe-area-inset-right);bottom:env(safe-area-inset-bottom);'
            document.body.appendChild(probe)
            const r = probe.getBoundingClientRect()
            probe.remove()
            const scroller =
              document.scrollingElement === document.documentElement
                ? 'document(html)'
                : document.scrollingElement?.tagName ?? '?'
            el.textContent =
              `inset-top=${r.top.toFixed(0)}  inset-bottom=${(window.innerHeight - r.bottom).toFixed(0)}\n` +
              `inset-left=${r.left.toFixed(0)}  inset-right=${(window.innerWidth - r.right).toFixed(0)}\n` +
              `innerHeight=${window.innerHeight}  dpr=${window.devicePixelRatio}\n` +
              `scrollingElement=${scroller}`
          }}
        />
      </header>

      <main className="mx-auto w-full max-w-md px-4 py-4 flex flex-col gap-3">
        {rows.map((n) => (
          <div key={n} className="rounded-lg border border-border bg-card px-4 py-6 text-sm">
            Row {n} of {rows.length}
          </div>
        ))}
      </main>

      <footer className="px-4 pb-16 pt-8 text-center text-xs text-muted-foreground">
        End of list.
      </footer>
    </div>
  )
}
