import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Alert } from '@ziee/kit'
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react'
import type { OverlayScrollbarsComponentRef } from 'overlayscrollbars-react'
import type { BundledLanguage, ShikiTransformer } from 'shiki'
import { useTheme } from '@/hooks/useTheme'
import {
  RAWCODE_CHUNK_LINES,
  RAWCODE_MAX_LINES,
  applyLineCap,
  buildPlainChunkHtml,
  chunkLineArray,
  chunkReservedHeight,
  type LineChunk,
} from './chunking'

// Lazy-load the shiki highlighter core (~90 KB, plus the bundled-grammar table)
// so it stays OUT of the initial entry chunk — it only loads when a code file is
// actually viewed. Module-level promise cache so repeated views reuse one import.
let shikiPromise: Promise<typeof import('shiki')> | null = null
function loadShiki(): Promise<typeof import('shiki')> {
  if (!shikiPromise) shikiPromise = import('shiki')
  return shikiPromise
}

/** File-ext → shiki bundled-language id. Most common extensions map
 *  directly (json, html, css, sql, java, c, cpp, etc.) — this map only
 *  covers the few that differ from shiki's canonical names. Unknown
 *  extensions render as plain text (no highlighting). */
const EXT_TO_LANG: Record<string, BundledLanguage> = {
  sh: 'bash',
  zsh: 'bash',
  py: 'python',
  rb: 'ruby',
  rs: 'rust',
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  yml: 'yaml',
  cs: 'csharp',
  pl: 'perl',
  hs: 'haskell',
  kt: 'kotlin',
  md: 'markdown',
  markdown: 'markdown',
  ini: 'ini',
  conf: 'ini',
}

function inferLang(filename?: string): string | null {
  if (!filename) return null
  const ext = filename.split('.').pop()?.toLowerCase() ?? ''
  if (ext in EXT_TO_LANG) return EXT_TO_LANG[ext]
  // The raw extension may itself be a valid shiki language id (json, css, sql,
  // java, c, cpp, php, swift, go, dart, …). We can no longer check the bundled-
  // language table synchronously (it lives behind the lazy shiki import), so we
  // return the ext as a candidate and validate it against `bundledLanguages`
  // inside the async highlight below — an unknown id falls back to plain text.
  return ext || null
}

/** Shiki transformer factory — for each `<span class="line">` shiki emits,
 *  restructure into exactly two children: a `.line-number` gutter span and a
 *  `.line-code` wrapper containing the original token children. The two become
 *  CSS-grid columns at render time (gutter + code); the gutter is
 *  `position: sticky; left: 0` so it stays anchored on horizontal scroll.
 *
 *  Because the view is highlighted CHUNK-BY-CHUNK (each chunk is a separate
 *  `codeToHtml` call over only its lines), the transformer takes the chunk's
 *  0-based `startLine` so the emitted gutter numbers stay GLOBAL (continuous
 *  across chunks) rather than restarting at 1 per chunk.
 *
 *  The wrapper around the tokens is load-bearing: without it the grid would put
 *  each token into its own grid cell, with extras overflowing into implicit
 *  rows — the broken-stacked layout that looks like every token got its own
 *  visual line. */
function makeLineNumberTransformer(startLine: number): ShikiTransformer {
  return {
    name: 'raw-code-view:line-numbers',
    // Strip the `tabindex="0"` shiki stamps on its <pre>: with one <pre> per
    // chunk it would add a keyboard tab-stop PER chunk. The single
    // OverlayScrollbars viewport is the scroll region (the plain-chunk builder
    // omits tabindex too — chunking.ts::buildPlainChunkHtml).
    pre(node) {
      if (node.properties) delete node.properties.tabindex
    },
    line(node, line) {
      const codeWrap = {
        type: 'element' as const,
        tagName: 'span',
        properties: { className: ['line-code'] },
        children: node.children,
      }
      node.children = [
        {
          type: 'element',
          tagName: 'span',
          properties: { className: ['line-number'] },
          // `line` is 1-based within this chunk's codeToHtml call; add the
          // chunk's 0-based startLine to recover the global 1-based number.
          children: [{ type: 'text', value: String(startLine + line) }],
        },
        codeWrap,
      ]
    },
  }
}

/** One chunk slot. Memoized on its PRIMITIVE props (html string + reserved px)
 *  so a single chunk highlighting (which replaces the parent's `highlighted`
 *  Map) does NOT re-render the other N-1 chunks' subtrees — only the chunk whose
 *  `html` actually changed re-renders. Offscreen chunks skip layout/paint via
 *  `content-visibility:auto`; their reserved height keeps the scrollbar accurate. */
const CodeChunk = memo(function CodeChunk({
  index,
  html,
  reservedPx,
}: {
  index: number
  html: string
  reservedPx: number
}) {
  return (
    <div
      data-chunk-index={index}
      style={{
        contentVisibility: 'auto',
        containIntrinsicSize: `auto ${reservedPx}px`,
      }}
      // eslint-disable-next-line react/no-danger
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
})

export function RawCodeView({
  text,
  filename,
  wordWrap = false,
}: {
  text: string
  /** Optional filename used to infer the syntax-highlight language.
   *  When omitted (or extension isn't a known shiki grammar), the
   *  body renders as plain text. */
  filename?: string
  /** When true, long lines soft-wrap instead of scrolling horizontally.
   *  Default false preserves the one-line-per-line + horizontal-scroll render. */
  wordWrap?: boolean
}) {
  const { isDarkMode } = useTheme()

  // Split → cap → chunk. All lines stay in the DOM (find-in-document walks DOM
  // text nodes); only the Shiki HIGHLIGHT of a chunk is deferred until it
  // scrolls into view (mirrors pdf/body.tsx's page-on-demand). Memoized on
  // `text` so unrelated re-renders don't re-split a large file.
  const { chunks, truncated, lang, plainHtml } = useMemo(() => {
    const all = text.split('\n')
    const { lines, truncated } = applyLineCap(all, RAWCODE_MAX_LINES)
    const chunks = chunkLineArray(lines, RAWCODE_CHUNK_LINES)
    return {
      chunks,
      truncated,
      lang: inferLang(filename),
      // Plain (un-highlighted) HTML for every chunk, built once. A chunk renders
      // this until it is highlighted; both have identical text, so find is stable.
      plainHtml: chunks.map(buildPlainChunkHtml),
    }
  }, [text, filename])

  // Highlighted HTML per chunk index. Populated lazily by the observer; a chunk
  // absent from this map renders its plain HTML. Reset whenever the source/theme
  // changes (below) so stale highlights never leak across files/themes.
  const [highlighted, setHighlighted] = useState<Map<number, string>>(new Map())
  // Shiki emits the theme background as an inline `background-color` on <pre>,
  // NOT as a CSS var — so `var(--shiki-bg)` was undefined and the sticky
  // line-number gutter was transparent (code scrolled visible underneath it on
  // x-scroll). Extract it from the first highlighted chunk and expose it as the
  // var so the gutter is opaque.
  const [shikiBg, setShikiBg] = useState<string | null>(null)

  // Resolved shiki entrypoints + validated language for the current file/theme.
  const readyRef = useRef<{
    codeToHtml: typeof import('shiki').codeToHtml
    validLang: BundledLanguage | 'text'
    theme: string
  } | null>(null)
  // Indices already highlighted or in-flight, so the observer never double-fetches
  // a chunk (mirrors the PDF store's per-page dedupe).
  const requestedRef = useRef<Set<number>>(new Set())
  const osRef = useRef<OverlayScrollbarsComponentRef>(null)
  // Monotonic generation, bumped on every (text/filename/theme) reset. Each
  // highlight request captures the current gen; a resolution whose gen is stale
  // (the file/theme changed while it was in-flight) is DISCARDED, so a previous
  // file's or theme's HTML can never land in the new state (cancel guard).
  const genRef = useRef(0)
  // Bumped once shiki is resolved for the current gen, so the observer effect
  // re-attaches after a reset (a theme flip clears highlights; re-observing makes
  // the IO fire its initial callback for currently-visible chunks → they
  // re-highlight without a scroll).
  const [readyGen, setReadyGen] = useState(0)

  // Highlight a single chunk (idempotent). Safe to call before shiki is ready —
  // it no-ops; the observer re-drives visible chunks once ready.
  const highlightChunk = useCallback(
    (index: number) => {
      const ready = readyRef.current
      if (!ready) return
      if (index < 0 || index >= chunks.length) return
      if (requestedRef.current.has(index)) return
      requestedRef.current.add(index)
      const gen = genRef.current
      const chunk: LineChunk = chunks[index]
      ready
        .codeToHtml(chunk.text, {
          lang: ready.validLang,
          theme: ready.theme,
          transformers: [makeLineNumberTransformer(chunk.startLine)],
        })
        .then(html => {
          // Stale resolution (file/theme changed while in-flight) → drop it.
          if (genRef.current !== gen) return
          setHighlighted(prev => {
            const next = new Map(prev)
            next.set(index, html)
            return next
          })
          setShikiBg(prev => prev ?? html.match(/background-color:\s*([^;"]+)/)?.[1] ?? null)
        })
        .catch(err => {
          // Leave the chunk on its plain-text render (the user still sees the
          // code, just without colors) and allow a later retry.
          if (genRef.current === gen) requestedRef.current.delete(index)
          // eslint-disable-next-line no-console
          console.warn('[RawCodeView] shiki highlight failed for chunk', index, err)
        })
    },
    [chunks],
  )

  // Resolve shiki + validate the language once per (text, filename, theme). On
  // any change, bump the generation + reset the highlight caches so a theme flip
  // / new file re-derives colors instead of showing the previous theme's spans
  // (and any in-flight highlight from the old gen is discarded on resolve).
  useEffect(() => {
    let cancelled = false
    genRef.current += 1
    readyRef.current = null
    requestedRef.current = new Set()
    setHighlighted(new Map())
    setShikiBg(null)
    loadShiki()
      .then(({ codeToHtml, bundledLanguages }) => {
        if (cancelled) return
        const validLang =
          lang && lang in bundledLanguages ? (lang as BundledLanguage) : 'text'
        // High-contrast themes for WCAG-AA (Round 3.1 contrast fix). The app's
        // theme is user-controlled (ThemeProvider), not OS prefers-color-scheme,
        // so pick the single matching theme rather than shiki's CSS-var mode.
        readyRef.current = {
          codeToHtml,
          validLang,
          theme: isDarkMode
            ? 'github-dark-high-contrast'
            : 'github-light-high-contrast',
        }
        // Signal the observer effect to (re)attach + highlight the visible chunks
        // for this generation. Eager highlighting lives there so it fires after
        // the OverlayScrollbars viewport exists (see below).
        setReadyGen(g => g + 1)
      })
      .catch(err => {
        // Shiki import failed entirely — every chunk stays on its plain-text
        // render. No highlighting, but the file is fully readable + findable.
        // eslint-disable-next-line no-console
        console.warn('[RawCodeView] shiki load failed:', err)
      })
    return () => {
      cancelled = true
    }
    // `text` (not just `lang`) is a dep: a same-language new file / new version
    // must also reset the caches. highlightChunk is intentionally NOT a dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text, lang, isDarkMode])

  // IntersectionObserver over the chunk slots — highlight a chunk (and the next,
  // as a small prefetch) as it scrolls into view (+ a 600px margin). The element
  // that actually scrolls (and must be the observer root) is the OverlayScrollbars
  // viewport, not the host. Re-runs when the chunks change OR shiki becomes ready
  // for a new generation (`readyGen`) so a theme flip re-highlights visible
  // chunks. Same page-on-demand shape as pdf/body.tsx (rootMargin + eager-first
  // differ). With OverlayScrollbars `defer`, the viewport may not exist on the
  // first run — retry on the next frame until it does (else a large first file
  // would only ever highlight the eager chunks).
  useEffect(() => {
    if (!readyRef.current || chunks.length === 0) return
    let cancelled = false
    let raf = 0
    let io: IntersectionObserver | null = null
    const attach = () => {
      if (cancelled) return
      const root = osRef.current?.osInstance()?.elements().viewport
      if (!root) {
        raf = requestAnimationFrame(attach)
        return
      }
      // Eager-highlight the first chunks (small files + the initial viewport).
      highlightChunk(0)
      highlightChunk(1)
      io = new IntersectionObserver(
        entries => {
          for (const entry of entries) {
            if (!entry.isIntersecting) continue
            const idx = Number((entry.target as HTMLElement).dataset.chunkIndex)
            if (Number.isNaN(idx)) continue
            highlightChunk(idx)
            highlightChunk(idx + 1)
          }
        },
        { root, rootMargin: '600px 0px' },
      )
      // observe() makes the IO fire an initial callback for already-intersecting
      // chunks, so visible chunks highlight without a scroll — also the re-attach
      // path that re-highlights the viewport after a theme reset.
      root.querySelectorAll('[data-chunk-index]').forEach(el => io!.observe(el))
    }
    attach()
    return () => {
      cancelled = true
      cancelAnimationFrame(raf)
      io?.disconnect()
    }
  }, [chunks, readyGen, highlightChunk])

  return (
    <div
      className={`flex flex-col w-full h-full${wordWrap ? ' raw-code-wrap' : ''}`}
      data-testid="raw-code-view"
      data-word-wrap={wordWrap ? 'on' : 'off'}
      style={{ ['--shiki-bg' as string]: shikiBg ?? 'var(--card)' }}
    >
      {truncated && (
        <Alert
          title={`Showing first ${RAWCODE_MAX_LINES.toLocaleString()} lines. Download the file to view all data.`}
          tone="warning"
          className="m-2 flex-shrink-0"
          data-testid="file-rawcode-truncated-alert"
        />
      )}
      {/* OverlayScrollbars for consistent themed scrollbar styling (matches the
          rest of the app via DivScrollY). Both axes enabled so long lines get a
          horizontal scrollbar anchored at the viewport edge. `defer` lets the
          component init after layout settles; the observer effect's rAF retry
          waits for the viewport this creates. */}
      <OverlayScrollbarsComponent
        ref={osRef}
        className="flex-1 min-h-0 w-full raw-code-view"
        options={{
          scrollbars: { autoHide: 'scroll' },
          // Wrapping removes horizontal overflow, so hide the x scrollbar then.
          overflow: { x: wordWrap ? 'hidden' : 'scroll', y: 'scroll' },
        }}
        defer
      >
        {/* The chunk column is the horizontal-scroll content: width:max-content so
            it grows to the widest chunk (min 100% of the viewport). py-2 restores
            the top/bottom breathing room the per-chunk `<pre>` no longer carries
            (pre padding is 0 so chunks butt together with no inter-chunk gaps). */}
        {/* Each chunk is a memoized slot: offscreen chunks skip layout/paint
            (content-visibility) while their reserved height keeps the scrollbar
            accurate. The heavy Shiki highlight is deferred by the observer above;
            the DOM text (plain OR highlighted) is ALWAYS present so
            find-in-document spans the whole file. The memo ensures one chunk
            highlighting doesn't re-render the others.
            ONE `tabindex=0` here (the per-chunk <pre>s carry none — see
            chunking.ts + the transformer `pre` hook) gives keyboard-only users a
            single focus target inside the scroll viewport so arrow keys scroll
            the code, without the one-tab-stop-per-chunk anti-pattern. */}
        <div
          className="raw-code-chunks"
          tabIndex={0}
          role="group"
          aria-label="File contents"
        >
          {chunks.map((chunk, i) => (
            <CodeChunk
              key={i}
              index={i}
              html={highlighted.get(i) ?? plainHtml[i]}
              reservedPx={chunkReservedHeight(chunk.lines.length, wordWrap)}
            />
          ))}
        </div>
      </OverlayScrollbarsComponent>
      {/* Scoped CSS — the .raw-code-view container is the horizontal scroll
          context, so the gutter's `position: sticky; left: 0` anchors to its left
          edge. Code rows use CSS Grid [44px gutter | 1fr code] so each row
          stretches the full width of the longest line (and the container
          minimum). */}
      <style>{`
        .raw-code-view .raw-code-chunks {
          padding: 8px 0;
          min-width: 100%;
          width: max-content;
        }
        .raw-code-view pre.shiki {
          margin: 0;
          /* Vertical padding moved to .raw-code-chunks so per-chunk <pre>s butt
             together with no gap; horizontal stays 0. */
          padding: 0;
          background: var(--shiki-bg) !important;
          font-size: 13px;
          line-height: 1.55;
          tab-size: 4;
          min-width: 100%;
          width: max-content;
          /* Collapse the literal \\n characters between line spans — each .line
             is already display: grid, which starts its own visual row. Without
             this, the inter-line newlines render as visible line breaks ON TOP of
             the grid line breaks, doubling the gap between lines. */
          white-space: normal;
        }
        .raw-code-view pre.shiki code {
          display: block;
          min-width: 100%;
          width: max-content;
          white-space: normal;
        }
        .raw-code-view pre.shiki .line {
          display: grid;
          /* minmax(max-content, 1fr) on the code column: track grows to at least
             the line's intrinsic width (so long lines overflow the container and
             trigger horizontal scroll), at most 1fr (so short lines stretch
             full-width for the hover row background). A plain 1fr would clamp
             every line to container width, hiding the horizontal scroll. */
          grid-template-columns: 44px minmax(max-content, 1fr);
          column-gap: 12px;
          min-width: 100%;
          width: max-content;
          min-height: 1.55em;
        }
        .raw-code-view pre.shiki .line-number {
          position: sticky;
          left: 0;
          background: var(--shiki-bg, var(--card));
          color: var(--shiki-gutter);
          text-align: right;
          padding-right: 4px;
          user-select: none;
          font-variant-numeric: tabular-nums;
          z-index: 1;
        }
        /* Code column — preserve whitespace + tabs so shiki tokens sit at their
           original column positions. */
        .raw-code-view pre.shiki .line-code {
          white-space: pre;
        }
        /* Word-wrap mode — long lines soft-wrap instead of overflowing. The code
           column collapses to a plain 1fr (no max-content), and the whole pre
           stops growing past the container, so there's no horizontal scroll. The
           sticky line-number gutter still anchors left. */
        .raw-code-wrap .raw-code-view .raw-code-chunks,
        .raw-code-wrap .raw-code-view pre.shiki,
        .raw-code-wrap .raw-code-view pre.shiki code,
        .raw-code-wrap .raw-code-view pre.shiki .line {
          width: auto;
          min-width: 0;
        }
        .raw-code-wrap .raw-code-view pre.shiki .line {
          grid-template-columns: 44px 1fr;
        }
        .raw-code-wrap .raw-code-view pre.shiki .line-code {
          white-space: pre-wrap;
          overflow-wrap: anywhere;
        }
      `}</style>
    </div>
  )
}
