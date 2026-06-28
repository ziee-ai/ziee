import { Spin } from 'antd'
import { Streamdown } from 'streamdown'
import { Component, createElement, type ComponentProps, type JSX, type ReactNode } from 'react'
import type { FileViewerSlotProps } from '../../types/viewer'
import { useFileTextContent, useFileViewMode } from '../shared/hooks'
import { useResourceLinkContent } from '../../hooks/useResourceLinkContent'
import { RawCodeView } from '../shared/RawCodeView'
import { getSource } from '../shared/source'
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
  if (!isLocalImageUrl(src)) return null
  return createElement('img', props)
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
          data-testid="streamdown-fallback"
        >
          {this.props.fallbackText}
        </pre>
      )
    }
    return this.props.children
  }
}

// Stable identity so Streamdown's prop-equality avoids re-renders.
const SAFE_IMG_COMPONENTS = { img: SafeImg }

// Hoisted to module scope — a literal `[...]` in the JSX below would create a
// fresh array reference on every render, defeating any prop-equality check
// Streamdown does internally for its (expensive) shiki syntax-highlighting.
const SHIKI_THEME: ComponentProps<typeof Streamdown>['shikiTheme'] = [
  'github-light',
  'github-dark',
]

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

  if (content === '__error__') {
    return (
      <div className="flex items-center justify-center h-full text-sm opacity-70 p-4">
        Failed to load file content.
      </div>
    )
  }
  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  if (file && mode === 'raw') {
    return <RawCodeView text={content} filename={file.filename} />
  }
  return (
    <div className="p-4 overflow-auto h-full">
      <StreamdownErrorBoundary fallbackText={content}>
        <Streamdown
          shikiTheme={SHIKI_THEME}
          urlTransform={streamdownUrlTransform}
          components={SAFE_IMG_COMPONENTS}
        >
          {content}
        </Streamdown>
      </StreamdownErrorBoundary>
    </div>
  )
}
