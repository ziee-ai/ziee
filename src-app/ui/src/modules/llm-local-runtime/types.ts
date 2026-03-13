export interface RuntimeVersion {
  id: string
  engine: 'llamacpp' | 'mistralrs'
  version: string
  platform: string
  arch: string
  backend: string
  binary_path: string
  is_default: boolean
  file_size_bytes: number
  created_at: string
}

export interface RuntimeDownloadRequest {
  engine: string
  version: string
  platform: string
  arch: string
  backend: string
}

// From backend - simplified response
export interface RuntimeUpdateCheckRaw {
  engine: string
  available_versions: string[]
}

// Enhanced type with computed properties
export interface RuntimeUpdateCheck extends RuntimeUpdateCheckRaw {
  current_version?: string
  latest_version: string
  has_updates: boolean
}

export type RuntimeEngine = 'llamacpp' | 'mistralrs'
export type RuntimePlatform = 'linux' | 'macos' | 'windows'
export type RuntimeArch = 'x86_64' | 'aarch64'
export type RuntimeBackend = 'cpu' | 'cuda' | 'metal'

// Store type declarations
import type { StoreProxy } from '@/core/stores'
import type { useRuntimeVersionStore } from './stores/RuntimeVersion.store'
import type { useRuntimeUpdateStore } from './stores/RuntimeUpdate.store'
import type { useRuntimeDownloadDrawerStore } from './stores/RuntimeDownloadDrawer.store'
import type { useRuntimeDeleteConfirmStore } from './stores/RuntimeDeleteConfirm.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    RuntimeVersion: StoreProxy<ReturnType<typeof useRuntimeVersionStore.getState>>
    RuntimeUpdate: StoreProxy<ReturnType<typeof useRuntimeUpdateStore.getState>>
    RuntimeDownloadDrawer: StoreProxy<ReturnType<typeof useRuntimeDownloadDrawerStore.getState>>
    RuntimeDeleteConfirm: StoreProxy<ReturnType<typeof useRuntimeDeleteConfirmStore.getState>>
  }
}
