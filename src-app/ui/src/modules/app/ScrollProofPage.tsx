import { useEffect } from 'react'

/**
 * Document-scroll proof (public, /scroll-proof) — mirrors asurascans.com:
 *  - the DOCUMENT (body/window) is the scroller (tall page, normal flow),
 *  - the header is position: relative (real content that scrolls UP under the
 *    notch), NOT fixed,
 *  - theme-color is a valid hex matching the header, so the iOS status-bar strip
 *    blends with the header.
 * No viewport-fit=cover (asura doesn't use it either). Load on-device and
 * compare to asura: does the header scroll up under the notch, with the status
 * bar matching?
 */
const HEADER = '#913FE2' // asura's purple, so the effect is unmistakable

export default function ScrollProofPage() {
  const rows = Array.from({ length: 60 }, (_, i) => i + 1)

  useEffect(() => {
    const meta = document.querySelector('meta[name="theme-color"]')
    const prev = meta?.getAttribute('content') ?? null
    meta?.setAttribute('content', HEADER)
    return () => {
      if (meta && prev != null) meta.setAttribute('content', prev)
    }
  }, [])

  return (
    <div className="min-h-dvh w-full bg-background text-foreground">
      {/* RELATIVE header (asura pattern) — scrolls away with the document. */}
      <header
        data-allow-custom-color
        className="relative z-10 flex flex-col justify-center h-14 px-4 text-white"
        style={{ background: HEADER }}
      >
        <h1 className="text-base font-semibold">Scroll proof (asura-style)</h1>
        <p className="text-[11px] opacity-90">
          Header is relative; theme-color = this purple. Scroll up/down.
        </p>
      </header>

      <main className="mx-auto w-full max-w-md px-4 py-4 flex flex-col gap-3">
        {rows.map((n) => (
          <div key={n} className="rounded-lg border border-border bg-card px-4 py-6 text-sm">
            Row {n} of {rows.length}
          </div>
        ))}
      </main>

      <footer className="px-4 py-10 text-center text-xs text-muted-foreground">
        End of list.
      </footer>
    </div>
  )
}
