import { Spin } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types/viewer'
import { useFileTextContent, useFileViewMode } from '../shared/hooks'
import { RawCodeView } from '../shared/RawCodeView'
import { FindableRegion } from '../shared/find/FindableRegion'

export function WebBody(props: FileViewerSlotProps) {
  // Web viewer is not inline-capable (XSS surface; deferred). Type guard
  // only — chat dispatcher won't reach here for source-shaped props.
  if (!('file' in props)) return null
  const { file } = props
  const content = useFileTextContent(file)
  const mode = useFileViewMode(file.id)
  const wordWrap = Stores.File.fileWordWrap.get(file.id) ?? false

  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin label="Loading" /></div>
  }
  if (mode === 'raw') {
    // Find/word-wrap operate on the raw source (the rendered branch below is a
    // sandboxed iframe — a separate document our highlight can't reach).
    return (
      <FindableRegion fileId={file.id}>
        <RawCodeView text={content} filename={file.filename} wordWrap={wordWrap} />
      </FindableRegion>
    )
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
      className="w-full h-full border-none"
      style={{ minHeight: 400 }}
      title="preview"
    />
  )
}
