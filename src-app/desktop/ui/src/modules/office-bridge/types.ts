import type { OpenDoc } from '@/api-client/types'
import type { StoreProxy } from '@/core/stores'
import type { useOfficeBridgeStore } from './stores/OfficeBridge.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    OfficeBridge: StoreProxy<ReturnType<typeof useOfficeBridgeStore.getState>>
  }
}

/** Serializable right-panel tab data for the "Open Office documents" panel.
 *  Carries the last-known open-document snapshot so the panel renders
 *  immediately when opened from the tool-result card (and survives the
 *  conversation panel-snapshot rehydrate); the OfficeBridge store keeps it live
 *  by pushing each `sync:office_document` refetch in via `updateRightPanelTab`. */
export interface OpenDocumentsPanelData {
  documents: OpenDoc[]
}

declare module '@/modules/chat/core/stores/Chat.store' {
  interface PanelRendererMap {
    'office-bridge': OpenDocumentsPanelData
  }
}
