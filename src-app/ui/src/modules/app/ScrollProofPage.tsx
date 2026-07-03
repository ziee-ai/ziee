/**
 * Document-scroll proof page (public, /scroll-proof).
 *
 * Renders a tall column in NORMAL document flow — no app shell, no inner
 * ScrollArea — so the <body>/window is the scroller. On iOS Safari this is what
 * triggers the toolbar-collapse-on-scroll + content-flowing-under-the-bar
 * behavior that the fixed 100dvh app shell can't. Load it on the phone to
 * confirm the mechanism before we invest in refactoring real pages.
 */
export default function ScrollProofPage() {
  const rows = Array.from({ length: 60 }, (_, i) => i + 1)
  return (
    <div className="min-h-dvh w-full bg-background text-foreground">
      {/* Sticky header — should slide up under the (collapsing) Safari bar as
          you scroll, and the rows should flow beneath both. */}
      <header className="sticky top-0 z-10 bg-card/90 backdrop-blur border-b border-border px-4 py-3">
        <h1 className="text-base font-semibold">Scroll proof</h1>
        <p className="text-xs text-muted-foreground">
          Scroll down: the Safari toolbar should shrink and content should pass under it.
        </p>
      </header>

      <main className="mx-auto w-full max-w-md px-4 py-4 flex flex-col gap-3">
        {rows.map(n => (
          <div
            key={n}
            className="rounded-lg border border-border bg-card px-4 py-6 text-sm"
          >
            Row {n} of {rows.length} — this column scrolls the document, not an
            inner box.
          </div>
        ))}
      </main>

      <footer className="px-4 py-8 text-center text-xs text-muted-foreground">
        End of list — if you got here by scrolling the page itself (not an inner
        panel), document scroll is working.
      </footer>
    </div>
  )
}
