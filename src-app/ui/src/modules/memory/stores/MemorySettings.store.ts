import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'

export interface UserMemorySettingsRow {
  user_id: string
  extraction_enabled: boolean
  retrieval_enabled: boolean
  max_memories: number
  retention_days: number | null
  extraction_model_id: string | null
  created_at: string
  updated_at: string
}

interface MemorySettingsStore {
  settings: UserMemorySettingsRow | null
  loading: boolean
  saving: boolean
  error: string | null

  load: () => Promise<void>
  update: (
    patch: Partial<{
      extraction_enabled: boolean
      retrieval_enabled: boolean
      max_memories: number
      retention_days: number | null
      extraction_model_id: string | null
    }>,
  ) => Promise<UserMemorySettingsRow | null>
}

export const useMemorySettingsStore = create<MemorySettingsStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      loading: false,
      saving: false,
      error: null,

      load: async () => {
        set((d) => {
          d.loading = true
          d.error = null
        })
        try {
          const res = await fetch('/api/memory/settings', {
            credentials: 'include',
          })
          if (!res.ok) throw new Error(`Failed to load settings: ${res.status}`)
          const row: UserMemorySettingsRow = await res.json()
          set((d) => {
            d.settings = row
            d.loading = false
          })
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Failed to load settings'
            d.loading = false
          })
        }
      },

      update: async (patch) => {
        set((d) => {
          d.saving = true
          d.error = null
        })
        try {
          const res = await fetch('/api/memory/settings', {
            method: 'PUT',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(patch),
          })
          if (!res.ok) throw new Error(`Update failed: ${res.status}`)
          const row: UserMemorySettingsRow = await res.json()
          set((d) => {
            d.settings = row
            d.saving = false
          })
          return row
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Update failed'
            d.saving = false
          })
          return null
        }
      },
    })),
  ),
)

export const MemorySettingsStoreProxy = createStoreProxy(useMemorySettingsStore)
