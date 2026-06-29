import { Spin } from '@/components/ui'
import type { FileViewerSlotProps } from '../../types/viewer'
import { useFileTextContent } from '../shared/hooks'
import { useResourceLinkContent } from '../../hooks/useResourceLinkContent'
import { RawCodeView } from '../shared/RawCodeView'
import { getSource } from '../shared/source'

export function TextBody(props: FileViewerSlotProps) {
  const { file, url } = getSource(props)
  const rightPanelContent = useFileTextContent(file, !file)
  const inlineContent = useResourceLinkContent(url, !!file)
  const content = file ? rightPanelContent : inlineContent

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
  return <RawCodeView text={content} filename={file?.filename} />
}
