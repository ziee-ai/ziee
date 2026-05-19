import type { File as FileEntity } from '@/api-client/types'
import type { ReactNode } from 'react'

export interface FileViewRendererProps {
  file: FileEntity
}

export interface FileViewerEntry {
  render: (props: FileViewRendererProps) => ReactNode
  label: string        // used in FileCard subtitle
  compilable: boolean  // viewer decides: true = show Eye/Code toggle
  canCopy: boolean     // drives Copy button in header
}

export interface FileViewerModule {
  canHandle: (filename: string, mimeType?: string) => boolean
  entry: FileViewerEntry
}
