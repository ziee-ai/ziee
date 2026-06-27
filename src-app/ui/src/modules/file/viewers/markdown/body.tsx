import { Spin } from '@/components/ui'
import { Streamdown } from 'streamdown'
import type { ComponentProps } from 'react'
import type { FileViewerSlotProps } from '../../types/viewer'
import { useFileTextContent, useFileViewMode } from '../shared/hooks'
import { useResourceLinkContent } from '../../hooks/useResourceLinkContent'
import { RawCodeView } from '../shared/RawCodeView'
import { getSource } from '../shared/source'
import { StreamdownErrorBoundary } from '@/modules/chat/core/utils/StreamdownErrorBoundary'
import { streamdownUrlTransform, SafeImg } from '@/modules/chat/core/utils/streamdownUrlTransform'

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
    return <div className="flex items-center justify-center h-full"><Spin label="Loading" /></div>
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
