import { ScrollArea, Spin } from '@/components/ui'
import { MarkdownTable } from '@/components/common/MarkdownTable'
import {
  nodeToText,
  slugifyHeading,
  safeDecode,
  HEADING_CLASS,
  LINK_CLASS,
} from '@/components/common/markdownHeadings'
import { cn } from '@/lib/utils'
import { renderGfmAlert } from '@/components/common/gfmAlert'
import { BlockedImage } from '@/components/common/BlockedImage'
import { preprocessMarkdown } from '@/components/common/markdownPreprocess'
import { Streamdown } from '@/modules/chat/core/utils/LazyStreamdown'
import { Component, createElement, type ComponentProps, type JSX, type ReactNode } from 'react'
import type { FileViewerSlotProps } from '../../types/viewer'
import { Stores } from '@/core/stores'
import { useFileTextContent, useFileViewMode } from '../shared/hooks'
import { useResourceLinkContent } from '../../hooks/useResourceLinkContent'
import { RawCodeView } from '../shared/RawCodeView'
import { FindableRegion } from '../shared/find/FindableRegion'
import { getSource } from '../shared/source'
import { STREAMDOWN_PLUGINS } from '@/components/common/streamdownPlugins'
// ----- Inlined from @/modules/chat/core/utils/ (generic utilities, no chat deps) -----

function isLocalImageUrl(url: string): boolean {
  if (!url) return false
  if (url.startsWith('/')) return true
  if (url.startsWith('data:')) return false
  try {
    const u = new URL(url, window.location.origin)
    return u.origin === window.location.origin
  } catch {
    return false
  }
}

export function streamdownUrlTransform(url: string, key: string): string {
  if (key !== 'src') return url
  return isLocalImageUrl(url) ? url : ''
}

export function SafeImg(props: JSX.IntrinsicElements['img']) {
  const src = typeof props.src === 'string' ? props.src : ''
  // Blocked (non-same-origin / data:) — placeholder instead of nothing.
  if (!isLocalImageUrl(src)) {
    return <BlockedImage src={src} alt={typeof props.alt === 'string' ? props.alt : undefined} />
  }
  return createElement('img', props)
}

// GitHub-style slug id on each heading so in-file hash links (`[Setup](#setup)`)
// resolve. A single document, so unscoped ids are fine.
function makeHeading(level: 1 | 2 | 3 | 4 | 5 | 6) {
  return function Heading(props: JSX.IntrinsicElements['h1']) {
    const slug = slugifyHeading(nodeToText(props.children))
    return createElement(`h${level}`, {
      ...props,
      id: props.id ?? (slug || undefined),
      // Re-apply Streamdown's default heading class (overriding drops it).
      className: cn(HEADING_CLASS[level], props.className),
    })
  }
}

// Anchor override: for a `#hash` link, scroll to the matching heading instead of
// letting Streamdown's DEFAULT anchor pop its link-safety modal (which fires for
// EVERY link, hash anchors included). External links open in a new tab.
function MdAnchor(props: JSX.IntrinsicElements['a']) {
  const { href, children, className, ...rest } = props
  // Re-apply Streamdown's default link class (overriding drops accent + underline).
  const cls = cn(LINK_CLASS, className)
  if (href?.startsWith('#')) {
    const targetId = slugifyHeading(safeDecode(href.slice(1)))
    return (
      <a
        {...rest}
        className={cls}
        href={`#${targetId}`}
        onClick={(e) => {
          e.preventDefault()
          document
            .getElementById(targetId)
            ?.scrollIntoView({ behavior: 'smooth', block: 'start' })
        }}
      >
        {children}
      </a>
    )
  }
  return (
    <a {...rest} className={cls} href={href} target="_blank" rel="noreferrer">
      {children}
    </a>
  )
}

const isDynamicImportError = (err: unknown): boolean => {
  if (!(err instanceof Error)) return false
  const m = err.message ?? ''
  return (
    m.includes('Failed to fetch dynamically imported module') ||
    m.includes('Importing a module script failed') ||
    m.includes('Outdated Optimize Dep')
  )
}

interface StreamdownErrorBoundaryProps {
  fallbackText: string
  children: ReactNode
}

interface StreamdownErrorBoundaryState {
  error: Error | null
  retryAttempt: number
}

export class StreamdownErrorBoundary extends Component<StreamdownErrorBoundaryProps, StreamdownErrorBoundaryState> {
  state: StreamdownErrorBoundaryState = { error: null, retryAttempt: 0 }
  private retryTimer: ReturnType<typeof setTimeout> | null = null

  static getDerivedStateFromError(error: Error): Partial<StreamdownErrorBoundaryState> {
    return { error }
  }

  componentDidUpdate(_prevProps: StreamdownErrorBoundaryProps, prevState: StreamdownErrorBoundaryState) {
    if (
      this.state.error &&
      !prevState.error &&
      this.state.retryAttempt === 0 &&
      isDynamicImportError(this.state.error)
    ) {
      this.retryTimer = setTimeout(() => {
        this.setState({ error: null, retryAttempt: 1 })
      }, 1500)
    }
  }

  componentWillUnmount() {
    if (this.retryTimer) clearTimeout(this.retryTimer)
  }

  render() {
    if (this.state.error) {
      return (
        <pre
          className="whitespace-pre-wrap break-words p-2 text-sm opacity-80"
          data-testid="file-markdown-streamdown-fallback"
        >
          {this.props.fallbackText}
        </pre>
      )
    }
    return this.props.children
  }
}

// Stable identity so Streamdown's prop-equality avoids re-renders.
// `table` uses our wrapper too, so tables in the file viewer get OverlayScrollbars
// + an in-page fullscreen at z-[1200] (above the file drawer's z-1050) instead of
// Streamdown's native scroller + z-50 fullscreen (which hid behind the drawer).
// `> [!NOTE]`-style GFM alerts render as styled callouts; other blockquotes
// stay as plain blockquotes (the file viewer has no "Cited excerpt" chrome).
function MdBlockquote(props: JSX.IntrinsicElements['blockquote']) {
  return renderGfmAlert(props.children) ?? <blockquote {...props} />
}

const SAFE_IMG_COMPONENTS = {
  img: SafeImg,
  table: MarkdownTable,
  a: MdAnchor,
  blockquote: MdBlockquote,
  h1: makeHeading(1),
  h2: makeHeading(2),
  h3: makeHeading(3),
  h4: makeHeading(4),
  h5: makeHeading(5),
  h6: makeHeading(6),
}

// Hoisted to module scope — a literal `[...]` in the JSX below would create a
// fresh array reference on every render, defeating any prop-equality check
// Streamdown does internally for its (expensive) shiki syntax-highlighting.
const SHIKI_THEME: ComponentProps<typeof Streamdown>['shikiTheme'] = [
  'github-light-high-contrast',
  'github-dark-high-contrast',
]

// Windowing note (file-viewer-virtualization): the RENDERED markdown path below
// is intentionally NOT windowed — it keeps the FilePanel 10 MB byte cap as its
// only boundary. `Streamdown` exposes no block-level windowing seam, and
// reliably splitting markdown into independently-renderable blocks (nested
// tables / lists / fenced code can span arbitrary lengths) is not clean, so
// windowing rendered markdown would risk correctness for little gain under the
// byte cap. The markdown RAW mode (below) delegates to `RawCodeView`, so it
// inherits that viewer's chunk-on-demand windowing + lifted line cap for free.
export function MarkdownBody(props: FileViewerSlotProps) {
  const { file, url } = getSource(props)

  // Right-panel: existing FileStore-based content fetch + view-mode toggle.
  // Inline: URL-keyed fetch via useResourceLinkContent; no view-mode toggle
  // (the inline preview is always "rendered"). Each hook self-skips in the
  // wrong context — both are called every render to satisfy rules-of-hooks.
  const rightPanelContent = useFileTextContent(file, !file)
  const inlineContent = useResourceLinkContent(url, !!file)
  const content = file ? rightPanelContent : inlineContent
  const mode = useFileViewMode(file?.id ?? '')
  const wordWrap = file ? Stores.File.fileWordWrap.get(file.id) ?? false : false

  if (content === '__error__') {
    return (
      <div className="flex items-center justify-center h-full text-sm opacity-70 p-4">
        Failed to load file content.
      </div>
    )
  }
  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin label="Loading" /></div>
  }

  // Find-in-document wraps the content in the right-panel context (needs a fileId
  // to coordinate the header toggle); the inline preview renders directly.
  const wrapFind = (node: ReactNode) =>
    file ? <FindableRegion fileId={file.id}>{node}</FindableRegion> : node

  if (file && mode === 'raw') {
    return wrapFind(
      <RawCodeView text={content} filename={file.filename} wordWrap={wordWrap} />,
    )
  }
  return wrapFind(
    <ScrollArea axis="both" className="h-full">
      <div className="p-4">
        <StreamdownErrorBoundary fallbackText={content}>
          <Streamdown
            shikiTheme={SHIKI_THEME}
            plugins={STREAMDOWN_PLUGINS}
            urlTransform={streamdownUrlTransform}
            components={SAFE_IMG_COMPONENTS}
          >
            {preprocessMarkdown(content)}
          </Streamdown>
        </StreamdownErrorBoundary>
      </div>
    </ScrollArea>,
  )
}
