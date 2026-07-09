//! knowledge_base chat extension (auto-discovered at modules/*/chat-extension/).
//!
//! Three integrations:
//!   1. Composer — a "+" dropdown item (KbMenuItem) to attach/detach KBs and a
//!      status-row chip strip (KbStatusRow) for the current selection.
//!   2. Lifecycle — hydrate the selection from the conversation's server-side
//!      attachments on load; transfer the pending (new-chat) selection once the
//!      conversation id is minted on first send. NOTE: nothing is injected into
//!      the send request — `search_knowledge` resolves the conversation's
//!      attached KBs server-side, so persistence (attach/detach) is all that's
//!      needed.
//!   3. Rendering — a `tool_result` renderer (SearchKnowledgeToolResultCard) that
//!      turns a `search_knowledge` result into a retrieval-transparency panel,
//!      plus a `kb_source` right-panel renderer for the cited document.
//!
//! The card carries a static `contentMatch` (claims only `search_knowledge`), so
//! the registry's co-ownership seam lets the literature/file catch-alls still
//! render their own `tool_result` blocks. Priority sits below them defensively.

import { BookOpen } from 'lucide-react'
import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { KbMenuItem } from './components/KbMenuItem'
import { KbStatusRow } from './components/KbStatusRow'
import { SearchKnowledgeToolResultCard } from './components/SearchKnowledgeToolResultCard'
import type { KbSourceData } from './components/KbSourcePanel'

// Augment the central PanelRendererMap so `displayInRightPanel({ type:
// 'kb_source', data })` and `registerPanelRenderer('kb_source', …)` type-check.
declare module '@/modules/chat/core/stores/Chat.store' {
  interface PanelRendererMap {
    kb_source: KbSourceData
  }
}

const knowledgeBaseExtension: ChatExtension = createExtension({
  name: 'knowledge-base',
  description: 'Knowledge base grounding: composer attach + retrieval transparency',
  // Below literature(75)/file(80). Ordering is defensive only — the card's
  // static `contentMatch` already scopes it to `search_knowledge` blocks.
  priority: 70,

  initialize: async () => {
    const { registerPanelRenderer } = await import('@/modules/chat/core/stores/Chat.store')
    const { KbSourcePanel } = await import('./components/KbSourcePanel')
    registerPanelRenderer('kb_source', {
      icon: <BookOpen />,
      component: KbSourcePanel,
    })
  },

  slots: {
    toolbar_plus_items: { component: KbMenuItem, order: 25 },
    toolbar_status: { component: KbStatusRow, order: 15 },
  },

  contentTypes: {
    tool_result: SearchKnowledgeToolResultCard,
  },

  onConversationLoad: async conversation => {
    const { Stores } = await import('@/core/stores')
    const store = Stores.KnowledgeBaseComposer
    store.setCurrentConversation(conversation.id)
    if (conversation.id) await store.loadForConversation(conversation.id)
  },

  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    const store = Stores.KnowledgeBaseComposer.$
    const conversation = Stores.Chat.$.conversation
    // New conversation just minted: persist the pending selection to it.
    if (conversation?.id && !store.currentConversationId) {
      await store.transferPending(conversation.id)
    }
    return {}
  },
})

export default knowledgeBaseExtension
