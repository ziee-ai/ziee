import { createModule } from '@/core'
import { useOfficeBridgeStore } from './stores/OfficeBridge.store'
// CRITICAL: enable store + panel-renderer type declaration merging
// (registers `Stores.OfficeBridge` and the `office-bridge` PanelRendererMap key).
import './types'

// The "Open Office documents" right-panel + the `list_open_documents`
// tool-result card register via the auto-discovered chat extension at
// modules/office-bridge/chat-extension/extension.tsx — no import needed here.

export default createModule({
  metadata: {
    name: 'office-bridge',
    version: '1.0.0',
    description: 'Open Office documents chat panel (live open/close via sync).',
  },
  // The store reads Stores.Chat at refetch time (to push into the open panel);
  // the chat module supplies the right-panel host.
  dependencies: ['chat'],
  stores: [{ name: 'OfficeBridge', store: useOfficeBridgeStore }],
})
