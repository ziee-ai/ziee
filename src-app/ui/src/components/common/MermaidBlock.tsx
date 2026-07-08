import { memo, useEffect, useId, useRef, useState } from 'react'
import { Copy, Download } from 'lucide-react'
import { Button, Segmented, message } from '@/components/ui'
import { useThemeOptional } from '@/components/ui/kit/theme'

/**
 * Replacement renderer for Streamdown's built-in mermaid rendering
 * (`code` fences with `language-mermaid`). Registered via Streamdown's
 * `plugins.renderers` extension point (see `streamdownPlugins.ts`), which the
 * library resolves BEFORE its own mermaid path — so this component owns the
 * whole mermaid block.
 *
 * Mirrors the sibling `MarkdownTable.tsx`: a `[data-streamdown="mermaid-block"]`
 * card with an always-visible header toolbar above a `bg-background` body.
 * Closes AFFORDANCE_MATRIX gaps G1 (source⇄render toggle) + G2 (copy source),
 * plus a download-SVG rider.
 *
 * The diagram renders by default; a Segmented toggle flips the body to the raw
 * source. The diagram is rendered whenever the fence is complete (regardless of
 * the active mode) so toggling is instant and download-SVG works in either mode.
 */

type Mode = 'render' | 'source'

export interface MermaidBlockProps {
  /** Raw mermaid source (the fence body). */
  code: string
  /** True while the code fence is still streaming — defers rendering. */
  isIncomplete?: boolean
  language?: string
  meta?: string
  /** Gallery-only: preset the initial view mode. Defaults to `render`. */
  defaultMode?: Mode
}

export const MermaidBlock = memo(function MermaidBlock({
  code,
  isIncomplete = false,
  defaultMode = 'render',
}: MermaidBlockProps) {
  const [mode, setMode] = useState<Mode>(defaultMode)
  const [svg, setSvg] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  // Follow the ACTUAL rendered theme. The ThemeProvider context is the source of
  // truth when present, but a Streamdown/MermaidBlock can render in a subtree that
  // lacks the provider; falling back to the `html.dark` class the provider sets
  // keeps mermaid's palette matching the page (otherwise it renders its light
  // 'default' theme — #333 edge labels — on a dark background, failing contrast).
  const ctxDark = useThemeOptional()?.isDark
  const isDark =
    ctxDark ??
    (typeof document !== 'undefined' &&
      document.documentElement.classList.contains('dark'))
  // A DOM-id-safe, per-instance base for mermaid's temp element (useId yields
  // `:r0:`-style ids that are invalid CSS ids / break mermaid). A per-render
  // sequence suffix makes each render's id unique so a stale in-flight render
  // can't collide on the same temp DOM id as its successor.
  const baseId = `mermaid-${useId().replace(/[^a-zA-Z0-9]/g, '')}`
  const renderSeq = useRef(0)
  const source = code.replace(/\n$/, '')
  const isEmpty = source.trim().length === 0

  // Render the diagram whenever the fence is complete — independent of `mode`
  // (DEC-6) so switching is instant and download works in source mode too. Lazy
  // `import('mermaid')` keeps the heavy dep off the main chat bundle (mirrors
  // Streamdown's own lazy mermaid chunk).
  useEffect(() => {
    if (isIncomplete || isEmpty) {
      setSvg(null)
      setError(null)
      return
    }
    let cancelled = false
    const renderId = `${baseId}-${(renderSeq.current += 1)}`
    void (async () => {
      try {
        const mermaid = (await import('mermaid')).default
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: 'strict',
          theme: isDark ? 'dark' : 'default',
          // mermaid's dark-theme edge-label background is a translucent light fill
          // (~rgb(88,88,88) composited) that leaves its light label text at 4.43:1 —
          // a hair under WCAG AA. Pin a solid dark label background so the light text
          // clears 4.5:1 comfortably; light theme keeps mermaid's defaults.
          ...(isDark
            ? { themeVariables: { edgeLabelBackground: '#1c2128' } }
            : {}),
        })
        const { svg: out } = await mermaid.render(renderId, source)
        if (!cancelled) {
          setSvg(out)
          setError(null)
        }
      } catch (e) {
        // On a parse failure mermaid can leave a stray temp node in <body>
        // (named `d<id>` or `<id>`); clean up both.
        document.getElementById(`d${renderId}`)?.remove()
        document.getElementById(renderId)?.remove()
        if (!cancelled) {
          setSvg(null)
          setError(e instanceof Error ? e.message : String(e))
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [source, isDark, isIncomplete, isEmpty, baseId])

  const copySource = async () => {
    try {
      await navigator.clipboard.writeText(source)
      message.success('Copied!')
    } catch {
      message.error('Failed to copy')
    }
  }

  const downloadSvg = () => {
    if (!svg) return
    const blob = new Blob([svg], { type: 'image/svg+xml' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'mermaid-diagram.svg'
    // Attach before click: some engines ignore a programmatic click on a
    // detached anchor for downloads. Remove it right after.
    document.body.appendChild(a)
    a.click()
    a.remove()
    // Defer revoke: revoking synchronously right after click() can abort the
    // download before the browser has read the blob (observed in Firefox).
    setTimeout(() => URL.revokeObjectURL(url), 0)
  }

  return (
    <div
      data-streamdown="mermaid-block"
      className="my-4 flex w-full flex-col gap-2 rounded-xl border border-border bg-sidebar p-2"
    >
      {/* min-h-8 (not h-8): on ≤480px the toolbar's WCAG tap-target children grow
          to 44px; grow the row to contain them instead of clipping. On-grid + no
          desktop change. */}
      <div className="flex min-h-8 items-center justify-between gap-2 text-muted-foreground text-xs">
        <Segmented
          size="sm"
          data-testid="mermaid-source-toggle"
          aria-label="Mermaid view mode"
          value={mode}
          onValueChange={(v) => setMode(v as Mode)}
          options={[
            { label: 'Diagram', value: 'render' },
            { label: 'Source', value: 'source' },
          ]}
        />
        <div className="flex items-center gap-0.5">
          <Button
            size="icon"
            variant="ghost"
            className="size-7"
            tooltip="Copy source"
            icon={<Copy className="size-3.5" />}
            onClick={copySource}
            data-testid="mermaid-copy-source-btn"
          />
          <Button
            size="icon"
            variant="ghost"
            className="size-7"
            tooltip="Download SVG"
            icon={<Download className="size-3.5" />}
            onClick={downloadSvg}
            disabled={!svg}
            data-testid="mermaid-download-svg-btn"
          />
        </div>
      </div>

      <div className="overflow-x-auto rounded-md border border-border bg-background p-3">
        {mode === 'source' ? (
          <pre className="overflow-x-auto text-sm" data-testid="mermaid-source-view">
            <code className="font-mono">{source}</code>
          </pre>
        ) : isEmpty ? (
          <div
            className="flex items-center justify-center py-6 text-muted-foreground text-sm"
            data-testid="mermaid-empty"
          >
            Empty diagram
          </div>
        ) : isIncomplete ? (
          <div
            className="flex items-center justify-center py-6 text-muted-foreground text-sm"
            role="status"
            data-testid="mermaid-rendering"
          >
            Rendering diagram…
          </div>
        ) : error ? (
          <div className="text-destructive text-sm" role="alert" data-testid="mermaid-error">
            <p className="font-medium">Failed to render diagram</p>
            <p className="mt-1 break-words font-mono text-xs">{error}</p>
            <p className="mt-2 text-muted-foreground">Switch to Source to view the code.</p>
          </div>
        ) : svg ? (
          <div
            className="flex justify-center [&_svg]:h-auto [&_svg]:max-w-full"
            role="img"
            aria-label="Mermaid diagram"
            data-testid="mermaid-diagram"
            // Trusted output of mermaid's strict-mode sanitizer (DEC-8): scripts
            // and HTML labels are stripped by mermaid before we inject.
            dangerouslySetInnerHTML={{ __html: svg }}
          />
        ) : (
          <div
            className="flex items-center justify-center py-6 text-muted-foreground text-sm"
            role="status"
            data-testid="mermaid-rendering"
          >
            Rendering diagram…
          </div>
        )}
      </div>
    </div>
  )
})
