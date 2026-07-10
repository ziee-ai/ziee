import type { StoreProxy } from '@/core/stores'
import type { useJsToolSettingsStore } from './stores/JsToolSettings.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    JsToolSettings: StoreProxy<
      ReturnType<typeof useJsToolSettingsStore.getState>
    >
  }
}

export {}
