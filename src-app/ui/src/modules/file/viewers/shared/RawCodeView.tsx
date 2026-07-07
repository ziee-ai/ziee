import { useEffect, useMemo, useState } from 'react'
import { Alert } from '@/components/ui'
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react'
import type { BundledLanguage, ShikiTransformer } from 'shiki'
import { useTheme } from '@/hooks/useTheme'

// Lazy-load the shiki highlighter core (~90 KB, plus the bundled-grammar table)
// so it stays OUT of the initial entry chunk — it only loads when a code file is
// actually viewed. Module-level promise cache so repeated views reuse one import.
let shikiPromise: Promise<typeof import('shiki')> | null = null
function loadShiki(): Promise<typeof import('shiki')> {
  if (!shikiPromise) shikiPromise = import('shiki')
  return shikiPromise
}

/** Cap on rendered lines. Above this, the file is truncated to the first
 *  N and a banner offers Download for full content. The wider 10 MB
 *  byte-cap at FilePanel still applies upstream. */
const MAX_LINES = 10_000

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

/** Shiki transformer — for each `<span class="line">` it emits,
 *  restructure into exactly two children: a `.line-number` gutter
 *  span and a `.line-code` wrapper containing the original token
 *  children. The two become CSS-grid columns at render time (gutter
 *  + code), and the gutter becomes `position: sticky; left: 0` so
 *  it stays anchored when the user horizontally-scrolls long lines.
 *
 *  The wrapper around the tokens is load-bearing: without it the
 *  grid would put each token (`RhpcBLASctl`, `blas_set`, etc.) into
 *  its own grid cell, with extras overflowing into implicit rows —
 *  the broken-stacked layout that looks like every token got its
 *  own visual line.
 *
 *  Done via the transformer API (not a regex post-pass) so we get a
 *  clean hast AST instead of brittle nested-</span> parsing. */
const lineNumberTransformer: ShikiTransformer = {
  name: 'raw-code-view:line-numbers',
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
        children: [{ type: 'text', value: String(line) }],
      },
      codeWrap,
    ]
  },
}

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

  const { source, truncated, lang } = useMemo(() => {
    const allLines = text.split('\n')
    const wasTruncated = allLines.length > MAX_LINES
    const lines = wasTruncated ? allLines.slice(0, MAX_LINES) : allLines
    return {
      source: lines.join('\n'),
      truncated: wasTruncated,
      lang: inferLang(filename),
    }
  }, [text, filename])

  const [html, setHtml] = useState<string | null>(null)
  // Shiki emits the theme background as an inline `background-color` on <pre>,
  // NOT as a CSS var — so `var(--shiki-bg)` was undefined and the sticky
  // line-number gutter was transparent (code scrolled visible underneath it on
  // x-scroll). Extract it and expose it as the var so the gutter is opaque.
  const [shikiBg, setShikiBg] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    // codeToHtml lazy-loads only the requested grammar+theme on first
    // use — subsequent renders of the same (lang, theme) are cached
    // by shiki internally. We pick the single theme that matches the
    // app's current mode rather than emitting both via shiki's CSS-
    // variable mode — the app's theme is user-controlled via
    // ThemeProvider (not OS prefers-color-scheme), so a media-query
    // swap wouldn't reflect manual light/dark toggles.
    loadShiki()
      .then(({ codeToHtml, bundledLanguages }) => {
        // Validate the inferred language against the (now lazily-loaded)
        // bundled-grammar table; unknown ids render as plain text.
        const validLang =
          lang && lang in bundledLanguages ? (lang as BundledLanguage) : 'text'
        // High-contrast themes for WCAG-AA (Round 3.1 contrast fix).
        return codeToHtml(source, {
          lang: validLang,
          theme: isDarkMode
            ? 'github-dark-high-contrast'
            : 'github-light-high-contrast',
          transformers: [lineNumberTransformer],
        })
      })
      .then(out => {
        if (cancelled) return
        setHtml(out)
        const bg = out.match(/background-color:\s*([^;"]+)/)?.[1]
        setShikiBg(bg ?? null)
      })
      .catch(err => {
        if (cancelled) return
        // Fall back to escaped plain text on highlighter failure
        // (unknown grammar, worker crash, etc.) — the user still
        // sees their file, just without colors.
        // eslint-disable-next-line no-console
        console.warn('[RawCodeView] shiki highlight failed:', err)
        setHtml(
          `<pre><code>${source
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')}</code></pre>`,
        )
      })
    return () => {
      cancelled = true
    }
  }, [source, lang, isDarkMode])

  return (
    <div
      className={`flex flex-col w-full h-full${wordWrap ? ' raw-code-wrap' : ''}`}
      data-testid="raw-code-view"
      data-word-wrap={wordWrap ? 'on' : 'off'}
      style={{ ['--shiki-bg' as string]: shikiBg ?? 'var(--card)' }}
    >
      {truncated && (
        <Alert
          title={`Showing first ${MAX_LINES.toLocaleString()} lines. Download the file to view all data.`}
          tone="warning"
          className="m-2 flex-shrink-0"
          data-testid="file-rawcode-truncated-alert"
        />
      )}
      {/* OverlayScrollbars for consistent themed scrollbar styling
          (matches the rest of the app via DivScrollY). Both axes
          enabled so long lines get a horizontal scrollbar anchored
          at the viewport edge. The `defer` option lets the component
          init after layout settles, which avoids a flash during the
          initial shiki async render. */}
      <OverlayScrollbarsComponent
        className="flex-1 min-h-0 w-full raw-code-view"
        options={{
          scrollbars: { autoHide: 'scroll' },
          // Wrapping removes horizontal overflow, so hide the x scrollbar then.
          overflow: { x: wordWrap ? 'hidden' : 'scroll', y: 'scroll' },
        }}
        defer
      >
        <div
          // eslint-disable-next-line react/no-danger
          dangerouslySetInnerHTML={html === null ? undefined : { __html: html }}
        />
      </OverlayScrollbarsComponent>
      {/* Scoped CSS — the .raw-code-view container is the horizontal
          scroll context, so the gutter's `position: sticky; left: 0`
          anchors to its left edge. Code rows use CSS Grid
          [44px gutter | 1fr code] so each row stretches the full
          width of the longest line (and the container minimum). */}
      <style>{`
        .raw-code-view pre.shiki {
          margin: 0;
          padding: 8px 0;
          background: var(--shiki-bg) !important;
          font-size: 13px;
          line-height: 1.55;
          tab-size: 4;
          min-width: 100%;
          width: max-content;
          /* Collapse the literal \\n characters shiki inserts between
             line spans — each .line is already display: grid, which
             starts its own visual row. Without this, the inter-line
             newlines render as visible line breaks ON TOP of the
             grid line breaks, doubling the gap between lines. */
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
          /* minmax(max-content, 1fr) on the code column: track grows
             to at least the line's intrinsic width (so long lines
             overflow the container and trigger horizontal scroll),
             at most 1fr (so short lines stretch full-width for the
             hover row background). A plain 1fr would clamp every
             line to container width, hiding the horizontal scroll
             entirely. */
          grid-template-columns: 44px minmax(max-content, 1fr);
          column-gap: 12px;
          min-width: 100%;
          width: max-content;
          min-height: 1.55em;
          /* Browser-native virtualization: skip painting + style
             work for lines that are offscreen. Each line is
             reserved as ~22px tall (line-height 1.55 × font-size 13px
             ≈ 20.15, rounded up) so scrollbar geometry stays
             accurate without measuring every line. Cuts initial
             paint cost for 10k-line files from ~all-lines to
             ~viewport-worth-of-lines. */
          content-visibility: auto;
          contain-intrinsic-size: auto 22px;
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
        /* Code column — preserve whitespace + tabs so shiki tokens
           sit at their original column positions. The tokens
           themselves are inline-by-default; whitespace handling needs
           to be on this wrapper to apply to all of them. */
        .raw-code-view pre.shiki .line-code {
          white-space: pre;
        }
        /* Word-wrap mode — long lines soft-wrap instead of overflowing. The
           code column collapses to a plain 1fr (no max-content), and the whole
           pre stops growing past the container, so there's no horizontal scroll.
           The sticky line-number gutter still anchors left. */
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
