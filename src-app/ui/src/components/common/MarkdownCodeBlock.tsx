import { lazy, Suspense, type JSX, type ReactNode } from 'react'
import { Tooltip } from '@/components/ui'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { nodeToText } from '@/components/common/markdownHeadings'

// Reuse Streamdown's OWN code-block primitives (same lazy-chunk trick as
// MarkdownTable) so this file carries no static `import … from 'streamdown'`
// and the whole Shiki/markdown chunk stays split out of the entry bundle. Safe
// because this only ever mounts inside an already-loaded Streamdown tree.
const loadCodeBlock = lazyWithPreload(() =>
  import('streamdown').then(m => ({ default: m.CodeBlock })),
)
const loadCopyButton = lazyWithPreload(() =>
  import('streamdown').then(m => ({ default: m.CodeBlockCopyButton })),
)
const loadDownloadButton = lazyWithPreload(() =>
  import('streamdown').then(m => ({ default: m.CodeBlockDownloadButton })),
)
const CodeBlock = lazy(loadCodeBlock)
const CodeBlockCopyButton = lazy(loadCopyButton)
const CodeBlockDownloadButton = lazy(loadDownloadButton)

type PreProps = JSX.IntrinsicElements['pre'] & { node?: unknown }

/**
 * Override for Streamdown's `pre` (its default code-block renderer). Identical
 * output — Streamdown's own `CodeBlock` does the highlighting + header — but the
 * copy / download controls are wrapped in the app's styled kit `Tooltip` instead
 * of only carrying a native `title` (which reads as "no tooltip" next to every
 * other instant styled tooltip in the app).
 */
export function MarkdownCodeBlock({ children }: PreProps) {
  // The <pre> override receives the <code> element as its single child.
  const codeEl = Array.isArray(children) ? children[0] : children
  const codeProps =
    codeEl && typeof codeEl === 'object' && 'props' in codeEl
      ? (codeEl as { props?: { className?: string; children?: ReactNode } }).props
      : undefined
  const language = /language-([\w-]+)/.exec(codeProps?.className ?? '')?.[1] ?? ''
  // Trailing newline is markdown's fence terminator, not part of the code.
  const code = nodeToText(codeProps?.children).replace(/\n$/, '')

  // Nothing extractable (an unusual <pre> without a <code> child) → fall back to
  // the raw block so we never blank content.
  if (!code) return <pre>{children}</pre>

  return (
    <Suspense
      fallback={
        <pre className="overflow-x-auto rounded-md bg-muted p-3 text-sm">
          {code}
        </pre>
      }
    >
      <CodeBlock code={code} language={language}>
        {/* span trigger forwards the ref the kit Tooltip's asChild needs; the
            streamdown buttons keep their own default styling + icons. */}
        <Tooltip title="Copy code">
          <span className="inline-flex">
            <CodeBlockCopyButton code={code} />
          </span>
        </Tooltip>
        <Tooltip title="Download file">
          <span className="inline-flex">
            <CodeBlockDownloadButton code={code} language={language} />
          </span>
        </Tooltip>
      </CodeBlock>
    </Suspense>
  )
}
