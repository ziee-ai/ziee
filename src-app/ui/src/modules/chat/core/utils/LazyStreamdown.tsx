import { lazy, Suspense } from 'react'
import type { ComponentType } from 'react'
import type { StreamdownProps } from 'streamdown'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

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
 * ## Plugins ride the lazy chunk too (the `variant` prop)
 *
 * Streamdown's optional `@streamdown/code` (Shiki) and `@streamdown/math`
 * (KaTeX) plugins are ~730 KB raw / ~152 KB gzip. They are only ever USED
 * inside this lazy chunk, but they used to be built at module scope in
 * `streamdownPlugins.ts` / `chatMarkdownPlugins.ts` and passed in as a
 * `plugins` PROP by each (eagerly-bundled) call site — so their static import
 * chain pulled katex+shiki back into the entry chunk, defeating the whole point
 * of the lazy boundary.
 *
 * Instead, callers now pass a `variant` (`'chat'` adds the sandboxed-HTML
 * renderer; `'base'` is code+math+mermaid) and this wrapper resolves the
 * matching plugin object INSIDE the same dynamic `import()` that loads
 * `streamdown`. The katex+shiki graph therefore lands in the lazy chunk, not
 * the entry chunk. There are exactly two variants because there are exactly two
 * plugin sets in the app.
 *
 * This is a drop-in replacement: `import { Streamdown } from '@/modules/chat/core/utils/LazyStreamdown'`
 * in place of `import { Streamdown } from 'streamdown'`, plus a `variant`.
 *
 * The loader goes through core's `lazyWithPreload` (not a bare `import()`), so
 * the desktop override kicks in: inside the Tauri webview the chunk is already
 * embedded in the binary, so it preloads at module-load and `React.lazy`
 * resolves without ever showing the fallback. Remote-browser (tunnel) + web
 * builds keep the deferred behavior — the chunk only downloads on first render.
 */

export type StreamdownVariant = 'base' | 'chat'

/**
 * Build a lazy loader that imports `streamdown` AND the variant's plugin module
 * together, returning a component that injects `plugins`. Keeping the plugin
 * import inside this factory is what keeps katex/shiki out of the entry chunk.
 * A caller may still pass an explicit `plugins` prop to override the default.
 */
function makeVariantLoader(variant: StreamdownVariant) {
  return lazyWithPreload<ComponentType<StreamdownProps>>(async () => {
    if (variant === 'chat') {
      const [{ Streamdown: Impl }, { chatMarkdownPlugins }] = await Promise.all([
        import('streamdown'),
        import('@/modules/chat/core/utils/chatMarkdownPlugins'),
      ])
      const Wrapped: ComponentType<StreamdownProps> = props => (
        <Impl plugins={chatMarkdownPlugins} {...props} />
      )
      return { default: Wrapped }
    }
    const [{ Streamdown: Impl }, { STREAMDOWN_PLUGINS }] = await Promise.all([
      import('streamdown'),
      import('@/components/common/streamdownPlugins'),
    ])
    const Wrapped: ComponentType<StreamdownProps> = props => (
      <Impl plugins={STREAMDOWN_PLUGINS} {...props} />
    )
    return { default: Wrapped }
  })
}

// One stable loader identity per variant (so lazyWithPreload's promise cache +
// the desktop preload contract both key correctly).
const BaseImpl = lazy(makeVariantLoader('base'))
const ChatImpl = lazy(makeVariantLoader('chat'))

export function Streamdown({
  variant = 'base',
  ...props
}: StreamdownProps & { variant?: StreamdownVariant }) {
  const Impl = variant === 'chat' ? ChatImpl : BaseImpl
  const fallback =
    typeof props.children === 'string' ? (
      <div style={{ whiteSpace: 'pre-wrap' }}>{props.children}</div>
    ) : null

  return (
    <Suspense fallback={fallback}>
      <Impl {...props} />
    </Suspense>
  )
}
