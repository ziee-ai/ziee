import type { StoreProxy } from '@ziee/framework/stores'
import type { useJsToolSettingsStore } from './stores/JsToolSettings.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    JsToolSettings: StoreProxy<
      ReturnType<typeof useJsToolSettingsStore.getState>
    >
  }
}

export {}
