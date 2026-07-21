import type { StoreProxy } from '@ziee/framework/stores'
import type { useVoiceConfigStore } from './stores/voiceConfig'
import type { useVoiceDownloadProgressStore } from './stores/voiceDownloadProgress'
import type { useVoiceInstanceStore } from './stores/voiceInstance'
import type { useVoiceModelStore } from './stores/voiceModel'
import type { useVoiceModelDownloadProgressStore } from './stores/VoiceModelDownloadProgress.store'
import type { useVoiceModelUpdateStore } from './stores/voiceModelUpdate'
import type { useVoiceModelUploadStore } from './stores/voiceModelUpload'
import type { useVoiceRuntimeVersionStore } from './stores/voiceRuntimeVersion'
import type { useVoiceUpdateStore } from './stores/voiceUpdate'
import type { useVoiceUploadModelDrawerStore } from './stores/voiceUploadModelDrawer'

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
