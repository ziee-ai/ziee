import { Spin } from 'antd'
import type { FileViewerSlotProps } from '../../types'
import { useFileTextContent, useFileViewMode } from '../shared/hooks'
import { RawCodeView } from '../shared/RawCodeView'

export function WebBody({ file }: FileViewerSlotProps) {
  const content = useFileTextContent(file)
  const mode = useFileViewMode(file.id)

  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  if (mode === 'raw') {
    return <RawCodeView text={content} />
  }
  return (
    <iframe
      sandbox="allow-scripts"
      srcDoc={content}
      style={{ width: '100%', height: '100%', border: 'none', minHeight: 400 }}
      title="preview"
    />
  )
}
