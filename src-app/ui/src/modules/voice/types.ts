import type { StoreProxy } from '@/core/stores'
import type { useVoiceRuntimeVersionStore } from './stores/VoiceRuntimeVersion.store'
import type { useVoiceUpdateStore } from './stores/VoiceUpdate.store'
import type { useVoiceDownloadProgressStore } from './stores/VoiceDownloadProgress.store'
import type { useVoiceConfigStore } from './stores/VoiceConfig.store'
import type { useVoiceModelStore } from './stores/VoiceModel.store'
import type { useVoiceInstanceStore } from './stores/VoiceInstance.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    VoiceRuntimeVersion: StoreProxy<
      ReturnType<typeof useVoiceRuntimeVersionStore.getState>
    >
    VoiceUpdate: StoreProxy<ReturnType<typeof useVoiceUpdateStore.getState>>
    VoiceDownloadProgress: StoreProxy<
      ReturnType<typeof useVoiceDownloadProgressStore.getState>
    >
    VoiceConfig: StoreProxy<ReturnType<typeof useVoiceConfigStore.getState>>
    VoiceModel: StoreProxy<ReturnType<typeof useVoiceModelStore.getState>>
    VoiceInstance: StoreProxy<ReturnType<typeof useVoiceInstanceStore.getState>>
  }
}

export {}
