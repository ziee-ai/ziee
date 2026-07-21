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
import { KnowledgeBaseComposer } from '@/modules/knowledge-base/stores/knowledgeBaseComposer'

// Augment the central PanelRendererMap so `displayInRightPanel({ type:
// 'kb_source', data })` and `registerPanelRenderer('kb_source', …)` type-check.
declare module '@/modules/chat/core/stores/chat' {
  interface PanelRendererMap {
    kb_source: KbSourceData
  }
}

// Per-pane subscription teardown (ITEM-34/5), keyed by ctx.chatStore.
const paneKbSubs = new WeakMap<object, Array<() => void>>()

const knowledgeBaseExtension: ChatExtension = createExtension({
  name: 'knowledge-base',
  description: 'Knowledge base grounding: composer attach + retrieval transparency',
  // Below literature(75)/file(80). Ordering is defensive only — the card's
  // static `contentMatch` already scopes it to `search_knowledge` blocks.
  priority: 70,

  initialize: async (ctx) => {
    const { registerPanelRenderer } = await import(
      '@/modules/chat/core/stores/chat'
    )
    const { KbSourcePanel } = await import('./components/KbSourcePanel')
    registerPanelRenderer('kb_source', {
      icon: <BookOpen />,
      component: KbSourcePanel,
    })
    // Reset the composer selection when the active conversation changes to a
    // NEW (unsaved) chat — onConversationLoad only fires for EXISTING
    // conversations, so without this the pending buffer from conversation A
    // would leak into a fresh chat and get attached on first send. Binds to the
    // OWNING pane's chat store (ctx.chatStore, ITEM-34/5). A change to a real id
    // is handled by onConversationLoad (which re-hydrates from the server).
    const subs: Array<() => void> = []
    paneKbSubs.set(ctx.chatStore, subs)
    subs.push(
      ctx.chatStore.subscribe(
        (state: any) => state.conversation?.id,
        (id: string | undefined) => {
          // Reset only THIS pane's pending buffer (ITEM-51) — keyed by the pane's
          // own id — so one pane opening a new chat never wipes another pane's
          // buffered new-chat KB selection.
          if (!id) {
            const paneId = (ctx.chatStore.getState() as { paneId?: string | null }).paneId ?? null
            KnowledgeBaseComposer.resetPending(paneId)
          }
        },
      ),
    )
  },

  cleanup: async (ctx) => {
    const subs = paneKbSubs.get(ctx.chatStore)
    if (subs) {
      for (const unsub of subs) unsub()
      paneKbSubs.delete(ctx.chatStore)
    }
  },

  slots: {
    toolbar_plus_items: { component: KbMenuItem, order: 25 },
    toolbar_status: { component: KbStatusRow, order: 15 },
  },

  contentTypes: {
    tool_result: SearchKnowledgeToolResultCard,
  },

  onConversationLoad: async conversation => {
    const store = KnowledgeBaseComposer
    // Per-conversation (ITEM-46): hydrate THIS conversation's own slot.
    if (conversation.id) await store.loadForConversation(conversation.id)
    // Read-only KBs inherited from the conversation's project (scope legibility).
    void store.loadInheritedFor(
      conversation.id ?? null,
      (conversation as { project_id?: string | null }).project_id ?? null,
    )
  },

  onMessageSent: async ownerPaneId => {
    const { Stores } = await import('@ziee/framework/stores')
    const { pendingKbKey } = await import('../stores/kbSelectionKey')
    const { paneRegistry } = await import('@/modules/chat/core/stores/chatBridge')
    const snap = KnowledgeBaseComposer.$
    // Resolve the SENDING pane's conversation from the threaded `ownerPaneId`, NOT
    // a `Stores.Chat.$` read (which routes to the FOCUSED pane — in split view the
    // pane that sent may no longer be focused by the time this async hook runs, so
    // a `.$` read would transfer the pending buffer onto the wrong conversation or
    // short-circuit and silently drop it). Single-pane (no paneId) falls back to
    // the bridge, which is the sole/focused pane there.
    const paneState = ownerPaneId
      ? (paneRegistry.get(ownerPaneId)?.api.getState() as
          | { conversation?: { id?: string } }
          | undefined)
      : undefined
    const conversation = paneState?.conversation ?? Stores.Chat.$.conversation
    // A brand-new conversation (just minted, not yet hydrated into its own slot)
    // with a non-empty pending buffer → move THIS pane's pending selection under it
    // (ITEM-51: read the SENDING pane's own pending key). An existing conversation
    // already owns a slot (via onConversationLoad), so this no-ops for it.
    const pendingSize =
      snap.selectionByConversation.get(pendingKbKey(ownerPaneId))?.size ?? 0
    if (
      conversation?.id &&
      !snap.selectionByConversation.has(conversation.id) &&
      pendingSize > 0
    ) {
      await KnowledgeBaseComposer.transferPending(conversation.id, ownerPaneId)
    }
    return {}
  },
})

export default knowledgeBaseExtension
