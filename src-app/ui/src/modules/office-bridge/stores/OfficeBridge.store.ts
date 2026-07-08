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
    // Last refetch error message, surfaced by the panel's error branch. Cleared
    // on the next successful load so a recovered fetch drops the banner.
    error: null as string | null,
  },
  actions: set => {
    // Push the fresh list into the open right-panel tab (if any) so an
    // already-open panel updates live. No-op when the panel is closed.
    const pushToOpenPanel = (documents: OpenDoc[]) => {
      // `$` is the hook-free state-read escape; actions (updateRightPanelTab) are
      // callable directly (main removed the `__state` alias — see store-kit.ts).
      const tab = Stores.Chat.$.rightPanel.tabs.find(t => t.type === OFFICE_DOCS_PANEL_TYPE)
      if (tab) {
        Stores.Chat.updateRightPanelTab<'office-bridge'>(tab.id, { documents })
      }
    }

    const load = () =>
      refetchOpenDocuments({
        hasUsePermission: () => hasPermissionNow(Permissions.OfficeBridgeUse),
        fetchDocuments: () => ApiClient.OfficeBridge.listDocuments(),
        setDocuments: docs =>
          set(s => {
            s.documents = docs
            // A successful load clears any prior error.
            s.error = null
          }),
        setLoading: loading =>
          set(s => {
            s.loading = loading
          }),
        pushToOpenPanel,
        onError: err =>
          set(s => {
            s.error =
              err instanceof Error
                ? err.message
                : 'Failed to load open Office documents.'
          }),
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
