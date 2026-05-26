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
  // sandbox WITHOUT allow-scripts. Both file types (HTML and SVG) render
  // their visual content declaratively; script execution would be a real
  // XSS vector since file content comes from messageFilesCache, which
  // includes files from OTHER users in shared conversations. An attacker-
  // crafted SVG or HTML could phish or fetch external endpoints from the
  // viewer's same-origin context.
  //
  // If a future tool needs to render interactive HTML, gate that behind
  // an explicit "I trust this content" user action rather than letting
  // every uploaded file execute by default.
  return (
    <iframe
      sandbox=""
      srcDoc={content}
      style={{ width: '100%', height: '100%', border: 'none', minHeight: 400 }}
      title="preview"
    />
  )
}
