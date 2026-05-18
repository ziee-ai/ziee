import type { FileViewerModule, FileViewerEntry } from './types'

type ViewerModule = { viewers: FileViewerModule[] }

const modules = import.meta.glob<ViewerModule>('./file-viewers/*/module.ts', { eager: true })
const allViewers: FileViewerModule[] = Object.values(modules).flatMap(m => m.viewers)

export function getViewer(filename: string, mimeType?: string): FileViewerEntry | undefined {
  return allViewers.find(v => v.canHandle(filename, mimeType))?.entry
}
