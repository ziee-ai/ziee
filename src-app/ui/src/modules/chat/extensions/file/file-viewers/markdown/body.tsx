import { Spin } from 'antd'
import { Streamdown } from 'streamdown'
import type { ComponentProps } from 'react'
import type { FileViewerSlotProps } from '../../types'
import { useFileTextContent, useFileViewMode } from '../shared/hooks'
import { RawCodeView } from '../shared/RawCodeView'

// Hoisted to module scope — a literal `[...]` in the JSX below would create a
// fresh array reference on every render, defeating any prop-equality check
// Streamdown does internally for its (expensive) shiki syntax-highlighting.
const SHIKI_THEME: ComponentProps<typeof Streamdown>['shikiTheme'] = [
  'github-light',
  'github-dark',
]

export function MarkdownBody({ file }: FileViewerSlotProps) {
  const content = useFileTextContent(file)
  const mode = useFileViewMode(file.id)

  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  if (mode === 'raw') {
    return <RawCodeView text={content} />
  }
  return (
    <div className="p-4 overflow-auto h-full">
      <Streamdown shikiTheme={SHIKI_THEME}>{content}</Streamdown>
    </div>
  )
}
