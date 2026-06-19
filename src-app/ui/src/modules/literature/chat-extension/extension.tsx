//! lit_search chat extension (auto-discovered at modules/*/chat-extension/).
//!
//! Registers the `literature` right-panel renderer (the screening workbench) and
//! a `tool_result` content renderer (the inline "Open in screening" card for
//! `literature_search` results). The content-type registry is FIRST-WINS
//! (early-exit, not stacked), and the file extension also registers `tool_result`
//! — so this extension takes a lower `priority` number to win, and the card
//! delegates every non-literature block back to the file view (MessageFilesView).
//! See LiteratureToolResultCard.

import { FileSearchOutlined } from '@ant-design/icons'
import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { LiteratureToolResultCard } from '../components/LiteratureToolResultCard'
import '../types' // PanelRendererMap declaration merge for 'literature'

const literatureExtension: ChatExtension = createExtension({
  name: 'literature',
  description: 'Literature search: screening right-panel + tool-result card',
  // Below the file extension's 80 so this wins the `tool_result` content type
  // (the registry early-exits on the first renderer); the card delegates every
  // non-literature block back to the file view. See LiteratureToolResultCard.
  priority: 75,

  initialize: async () => {
    const { registerPanelRenderer } = await import('@/modules/chat/core/stores/Chat.store')
    const { LiteratureScreeningPanel } = await import('../components/LiteratureScreeningPanel')
    registerPanelRenderer('literature', {
      icon: <FileSearchOutlined />,
      component: LiteratureScreeningPanel,
    })
  },

  // Stacked tool_result renderer — renders only literature_search results.
  contentTypes: {
    tool_result: LiteratureToolResultCard,
  },
})

export default literatureExtension
