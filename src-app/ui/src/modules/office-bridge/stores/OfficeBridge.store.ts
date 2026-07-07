import { ApiClient } from '@/api-client'
import type { OpenDoc } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { defineStore } from '@/core/store-kit'
import {
  OFFICE_DOCS_PANEL_TYPE,
  refetchOpenDocuments,
} from './officeBridgeSync'

/**
 * Live list of the user's open Office documents, backing the "Open Office
 * documents" chat right-panel (ITEM-14).
 *
 * Notify-and-refetch (the sync module's wire contract): the backend emits an
 * owner-scoped `office_document` sync frame on open/close (DEC-7), carrying no
 * row data; this store refetches `GET /office-bridge/documents`. The refetch is
 * SELF-GATED on `office_bridge::use` (equal to the endpoint's read perm) so a
 * `sync:reconnect` — which fires for every store regardless of audience — never
 * 403s for a user who lacks the perm.
 */
export const OfficeBridge = defineStore('OfficeBridge', {
  immer: true,
  state: {
    documents: [] as OpenDoc[],
    loading: false,
  },
  actions: set => {
    // Push the fresh list into the open right-panel tab (if any) so an
    // already-open panel updates live. No-op when the panel is closed.
    const pushToOpenPanel = (documents: OpenDoc[]) => {
      const chat = Stores.Chat.__state
      const tab = chat.rightPanel.tabs.find(t => t.type === OFFICE_DOCS_PANEL_TYPE)
      if (tab) {
        chat.updateRightPanelTab<'office-bridge'>(tab.id, { documents })
      }
    }

    const load = () =>
      refetchOpenDocuments({
        hasUsePermission: () => hasPermissionNow(Permissions.OfficeBridgeUse),
        fetchDocuments: () => ApiClient.OfficeBridge.listDocuments(),
        setDocuments: docs =>
          set(s => {
            s.documents = docs
          }),
        setLoading: loading =>
          set(s => {
            s.loading = loading
          }),
        pushToOpenPanel,
      })

    return { load }
  },
  init: ({ on, actions }) => {
    const reload = () => void actions.load()
    on('sync:office_document', reload)
    on('sync:reconnect', reload)
    // Eager first load (self-gated) so the panel has data the moment it opens.
    void actions.load()
  },
})

export const useOfficeBridgeStore = OfficeBridge.store
