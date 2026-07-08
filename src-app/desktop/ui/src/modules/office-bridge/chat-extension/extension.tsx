//! office-bridge chat extension (auto-discovered at modules/*/chat-extension/).
//!
//! Registers the `office-bridge` right-panel renderer (the "Open Office
//! documents" panel) and a `tool_result` content renderer (the inline card for
//! `list_open_documents` results that opens the panel). The card claims ONLY its
//! own blocks via the renderer's static `contentMatch`, so every other
//! `tool_result` falls through to the file / literature / workflow renderers.

import { FileText } from 'lucide-react'
import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { OpenDocumentsToolResultCard } from '../components/OpenDocumentsToolResultCard'
import '../types' // PanelRendererMap declaration merge for 'office-bridge'

const officeBridgeExtension: ChatExtension = createExtension({
  name: 'office-bridge',
  description: 'Open Office documents: right-panel renderer + tool-result card',
  // Below workflow (74), literature (75) and file (80) so it's tried first, but
  // its `contentMatch` claims only `list_open_documents` — every other
  // tool_result block falls through to the next renderer.
  priority: 73,

  initialize: async () => {
    const { registerPanelRenderer } = await import('@/modules/chat/core/stores/Chat.store')
    const { OpenDocumentsPanel } = await import('../components/OpenDocumentsPanel')
    registerPanelRenderer('office-bridge', {
      icon: <FileText />,
      component: OpenDocumentsPanel,
    })
  },

  contentTypes: {
    tool_result: OpenDocumentsToolResultCard,
  },
})

export default officeBridgeExtension
