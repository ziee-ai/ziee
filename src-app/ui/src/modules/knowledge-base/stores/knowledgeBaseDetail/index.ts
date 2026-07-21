import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { knowledgeBaseDetailState, type KnowledgeBaseDetailState } from './state'
import type { Actions } from './actions.gen'

const KnowledgeBaseDetailDef = defineStore<KnowledgeBaseDetailState, Actions>(
  'KnowledgeBaseDetail',
  {
    immer: true,
    state: knowledgeBaseDetailState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ on, get, actions }) => {
      // Live per-document index status + external file deletes.
      const refreshOpen = () => {
        const id = get().kb?.id
        if (!id) return
        // Refresh the loaded window (not a page-1 reset) so live status updates
        // reach every loaded row without collapsing the user's paging.
        void actions.refreshLoadedDocuments(id)
        void actions.refreshKb(id)
      }
      // Usage (conversations/projects a KB is attached to) changes on attach/detach,
      // NOT while documents index — refresh it only on the KB entity + reconnect,
      // not on the per-document index-state stream (which fires per doc at scale).
      on('sync:knowledge_base', () => {
        const id = get().kb?.id
        if (id) void actions.loadUsage(id)
      })
      on('sync:file_index_state', refreshOpen)
      on('sync:knowledge_base_document', refreshOpen)
      on('sync:file', refreshOpen)
      on('sync:reconnect', refreshOpen)
    },
  },
)

/** Direct-handle proxy — `import { KnowledgeBaseDetail }; KnowledgeBaseDetail.kb` / `KnowledgeBaseDetail.load()`.
 *  Importing this file self-registers the store (so `KnowledgeBaseDetailStore` resolves too). */
export const KnowledgeBaseDetail = registerLazyStore(KnowledgeBaseDetailDef)
/** Raw zustand store (kept for the type augmentation + any raw consumer). */
export const useKnowledgeBaseDetailStore = KnowledgeBaseDetailDef.store
