import { lazy, Suspense } from 'react'
import type { StreamdownProps } from 'streamdown'

/**
 * Lazy-loaded wrapper around Streamdown.
 *
 * The `streamdown` package drags in the heavy markdown-rendering pipeline —
 * Shiki syntax highlighting (`@shikijs/core` + `vscode-textmate`), the
 * micromark/mdast/rehype parse chain, and `parse5` HTML parsing. Statically
 * importing the `Streamdown` component pulled ~300 KB (gzip) of that into the
 * initial entry chunk, even though it is only needed once a markdown surface
 * (a rendered assistant message, a file/skill/workflow markdown view) actually
 * mounts.
 *
 * Routing every `Streamdown` render through a `React.lazy` boundary moves the
 * whole pipeline into its own chunk that loads on first use and is cached
 * thereafter. Until the chunk resolves, the raw markdown text is shown as
 * pre-wrapped plain text — so there is no blank flash, and the content stays
 * readable even if the chunk fails to load.
 *
 * This is a drop-in replacement: `import { Streamdown } from '@/modules/chat/core/utils/LazyStreamdown'`
 * in place of `import { Streamdown } from 'streamdown'`.
 */
const StreamdownImpl = lazy(() =>
  import('streamdown').then(m => ({ default: m.Streamdown })),
)

export function Streamdown(props: StreamdownProps) {
  const fallback =
    typeof props.children === 'string' ? (
      <div style={{ whiteSpace: 'pre-wrap' }}>{props.children}</div>
    ) : null

  return (
    <Suspense fallback={fallback}>
      <StreamdownImpl {...props} />
    </Suspense>
  )
}
