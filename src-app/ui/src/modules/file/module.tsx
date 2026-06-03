import { createModule } from '@/core'
import { useFileStore } from './stores/File.store'
import './types'

/**
 * File module — top-level home for file-domain state, components,
 * viewers, and the file-viewer registry. No routes, no nav slots, no
 * admin pages: file is a chat-composer concern (chat-extension auto-
 * discovered at modules/file/chat-extension/) AND a cross-module
 * primitive (projects' knowledge drawer reuses FileCard at
 * modules/file/components/FileCard.tsx).
 *
 * Backend counterpart: modules/file/ + modules/file/chat_extension/.
 */
export default createModule({
  metadata: {
    name: 'file',
    version: '1.0.0',
    description: 'File storage, upload, preview and viewer registry',
  },
  dependencies: ['router'],
  stores: [
    { name: 'File', store: useFileStore },
  ],
})
