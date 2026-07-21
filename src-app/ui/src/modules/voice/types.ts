import type { StoreProxy } from '@ziee/framework/stores'
import type { useVoiceConfigStore } from './stores/voiceConfig'
import type { useVoiceDownloadProgressStore } from './stores/voiceDownloadProgress'
import type { useVoiceInstanceStore } from './stores/VoiceInstance.store'
import type { useVoiceModelStore } from './stores/VoiceModel.store'
import type { useVoiceModelDownloadProgressStore } from './stores/VoiceModelDownloadProgress.store'
import type { useVoiceModelUpdateStore } from './stores/VoiceModelUpdate.store'
import type { useVoiceModelUploadStore } from './stores/VoiceModelUpload.store'
import type { useVoiceRuntimeVersionStore } from './stores/voiceRuntimeVersion'
import type { useVoiceUpdateStore } from './stores/VoiceUpdate.store'
import type { useVoiceUploadModelDrawerStore } from './stores/VoiceUploadModelDrawer.store'

declare module '@ziee/framework/stores' {
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
    VoiceModelUpdate: StoreProxy<
      ReturnType<typeof useVoiceModelUpdateStore.getState>
    >
    VoiceModelDownloadProgress: StoreProxy<
      ReturnType<typeof useVoiceModelDownloadProgressStore.getState>
    >
    VoiceModelUpload: StoreProxy<
      ReturnType<typeof useVoiceModelUploadStore.getState>
    >
    VoiceUploadModelDrawer: StoreProxy<
      ReturnType<typeof useVoiceUploadModelDrawerStore.getState>
    >
    VoiceInstance: StoreProxy<ReturnType<typeof useVoiceInstanceStore.getState>>
  }
}

export {}
