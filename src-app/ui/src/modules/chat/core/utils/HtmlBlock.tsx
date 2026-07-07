import { useState } from 'react'
import type { CustomRendererProps } from 'streamdown'
import { Check, Copy as CopyIcon } from 'lucide-react'
import { Button, Segmented, message } from '@/components/ui'
import { cn } from '@/lib/utils'
import { SANDBOX, buildSandboxedSrcdoc } from './htmlBlockSandbox'

type ViewMode = 'code' | 'preview'

/**
 * Streamdown custom-renderer for fenced ```html code blocks in chat markdown
 * (registered via `plugins.renderers` — see `streamdownPlugins.ts`).
 *
 * Gives each HTML block a **Code | Preview** toggle:
 *   - **Code** (the DEFAULT — for safety): the highlighted-ish source. The user
 *     opts INTO rendering per block; untrusted HTML never auto-executes.
 *   - **Preview**: the HTML rendered inside a STRICTLY SANDBOXED iframe
 *     (`sandbox="allow-scripts"`, no `allow-same-origin`; `srcdoc`; injected CSP
 *     that blocks external network; no top-navigation/popups/forms). All posture
 *     lives in `htmlBlockSandbox.ts`.
 *
 * While the fence is still streaming (`isIncomplete`) the block is pinned to
 * Code and Preview is disabled, so a half-written tag/script can't be rendered.
 */
export function HtmlBlock({ code, isIncomplete }: CustomRendererProps) {
  const [mode, setMode] = useState<ViewMode>('code')
  const [copied, setCopied] = useState(false)

  // A block still streaming must never render — force Code, disable Preview.
  const effectiveMode: ViewMode = isIncomplete ? 'code' : mode

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      message.success('HTML copied to clipboard')
      setTimeout(() => setCopied(false), 2000)
    } catch {
      message.error('Failed to copy HTML')
    }
  }

  return (
    <div
      data-testid="html-block"
      className="relative mb-2 rounded-lg border border-border bg-muted/40 overflow-hidden"
    >
      {/* Header: language label · Code/Preview toggle · copy */}
      <div className="flex items-center justify-between gap-2 px-3 py-1.5 border-b border-border">
        <span className="text-xs text-muted-foreground">html</span>
        <div className="flex items-center gap-2">
          <Segmented
            data-testid="html-block-toggle"
            size="sm"
            value={effectiveMode}
            onValueChange={(v) => setMode(v as ViewMode)}
            aria-label="HTML block view mode"
            options={[
              { label: 'Code', value: 'code' },
              // Preview is disabled until the fence finishes streaming.
              { label: 'Preview', value: 'preview', disabled: isIncomplete },
            ]}
          />
          <Button
            data-testid="html-block-copy-btn"
            size="default"
            variant="ghost"
            icon={copied ? <Check /> : <CopyIcon />}
            onClick={handleCopy}
          >
            {copied ? 'Copied' : 'Copy'}
          </Button>
        </div>
      </div>

      {/* Body */}
      {effectiveMode === 'preview' ? (
        <iframe
          data-testid="html-block-preview"
          title="HTML preview (sandboxed)"
          // SECURITY: allow-scripts ONLY (null origin — no parent/cookie/storage
          // reach); srcdoc carries an injected CSP that blocks external network.
          sandbox={SANDBOX}
          srcDoc={buildSandboxedSrcdoc(code)}
          referrerPolicy="no-referrer"
          loading="lazy"
          className="w-full h-96 border-0 bg-white"
        />
      ) : (
        <pre
          data-testid="html-block-source"
          className={cn('p-3 overflow-x-auto text-sm')}
        >
          <code className="language-html">{code}</code>
        </pre>
      )}
    </div>
  )
}
