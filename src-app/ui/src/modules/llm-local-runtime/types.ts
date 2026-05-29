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

// One upstream release in the update-check diff (mirrors backend
// AvailableVersion): what's installed + whether the binary is published
// for this host.
export interface RuntimeAvailableVersion {
  version: string
  installed: boolean
  installed_backends: string[]
  binary_ready: boolean
  available_backends: string[]
  // Suitable backend artifact for this host (GPU-version major match).
  recommended_backend?: string
  prerelease: boolean
  published_at?: string
}

// From backend - releases diffed against installed, scoped to host platform/arch.
export interface RuntimeUpdateCheckRaw {
  engine: string
  platform: string
  arch: string
  versions: RuntimeAvailableVersion[]
}

// Enhanced type with computed properties
export interface RuntimeUpdateCheck extends RuntimeUpdateCheckRaw {
  current_version?: string
  latest_version: string
  has_updates: boolean
}

export type RuntimeEngine = 'llamacpp' | 'mistralrs'

// Store type declarations
import type { StoreProxy } from '@/core/stores'
import type { useRuntimeVersionStore } from './stores/RuntimeVersion.store'
import type { useRuntimeUpdateStore } from './stores/RuntimeUpdate.store'
import type { useRuntimeDownloadDrawerStore } from './stores/RuntimeDownloadDrawer.store'
import type { useRuntimeDeleteConfirmStore } from './stores/RuntimeDeleteConfirm.store'
import type { useRuntimeConfigStore } from './stores/RuntimeConfig.store'
import type { useRuntimeModelUsageStore } from './stores/RuntimeModelUsage.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    RuntimeVersion: StoreProxy<ReturnType<typeof useRuntimeVersionStore.getState>>
    RuntimeUpdate: StoreProxy<ReturnType<typeof useRuntimeUpdateStore.getState>>
    RuntimeDownloadDrawer: StoreProxy<ReturnType<typeof useRuntimeDownloadDrawerStore.getState>>
    RuntimeDeleteConfirm: StoreProxy<ReturnType<typeof useRuntimeDeleteConfirmStore.getState>>
    RuntimeConfig: StoreProxy<ReturnType<typeof useRuntimeConfigStore.getState>>
    RuntimeModelUsage: StoreProxy<ReturnType<typeof useRuntimeModelUsageStore.getState>>
  }
}
