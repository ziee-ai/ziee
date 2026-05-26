import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'
import { emitMemoryAdminSettingsUpdated } from '@/modules/memory/events'

export interface MemoryAdminSettingsRow {
  id: number
  embedding_model_id: string | null
  embedding_dimensions: number
  default_extraction_model_id: string | null
  default_top_k: number
  cosine_threshold: number
  enabled: boolean
  soft_delete_grace_days: number
  daily_extraction_quota: number
  summarize_after_n_messages: number
  summarizer_keep_recent: number
  updated_at: string
}

export interface EmbeddingCapableModelRow {
  id: string
  name: string
  display_name: string | null
  provider_id: string
}

interface MemoryAdminStore {
  settings: MemoryAdminSettingsRow | null
  availableModels: EmbeddingCapableModelRow[]
  loading: boolean
  saving: boolean
  loadingModels: boolean
  error: string | null

  load: () => Promise<void>
  loadEmbeddingCapableModels: () => Promise<void>
  update: (
    patch: Partial<{
      embedding_model_id: string | null
      default_extraction_model_id: string | null
      default_top_k: number
      cosine_threshold: number
      enabled: boolean
      soft_delete_grace_days: number
      daily_extraction_quota: number
      summarize_after_n_messages: number
      summarizer_keep_recent: number
    }>,
  ) => Promise<MemoryAdminSettingsRow | null>
}

export const useMemoryAdminStore = create<MemoryAdminStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      availableModels: [],
      loading: false,
      saving: false,
      loadingModels: false,
      error: null,

      load: async () => {
        set((d) => {
          d.loading = true
          d.error = null
        })
        try {
          const res = await fetch('/api/memory/admin-settings', {
            credentials: 'include',
          })
          if (!res.ok) throw new Error(`Failed to load admin settings: ${res.status}`)
          const row: MemoryAdminSettingsRow = await res.json()
          set((d) => {
            d.settings = row
            d.loading = false
          })
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Failed to load admin settings'
            d.loading = false
          })
        }
      },

      loadEmbeddingCapableModels: async () => {
        set((d) => {
          d.loadingModels = true
        })
        try {
          // Server-side capability filter — Phase 2 added the
          // `?capability=text_embedding` query param on /api/llm-models.
          const res = await fetch(
            '/api/llm-models?capability=text_embedding&page=1&per_page=200',
            { credentials: 'include' },
          )
          if (!res.ok) throw new Error(`Failed to load models: ${res.status}`)
          const body: any = await res.json()
          const rows: EmbeddingCapableModelRow[] = (body.models ?? body ?? []).map(
            (m: any) => ({
              id: m.id,
              name: m.name,
              display_name: m.display_name,
              provider_id: m.provider_id,
            }),
          )
          set((d) => {
            d.availableModels = rows
            d.loadingModels = false
          })
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Failed to load embedding models'
            d.loadingModels = false
          })
        }
      },

      update: async (patch) => {
        set((d) => {
          d.saving = true
          d.error = null
        })
        try {
          const res = await fetch('/api/memory/admin-settings', {
            method: 'PUT',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(patch),
          })
          if (!res.ok) throw new Error(`Update failed: ${res.status}`)
          const row: MemoryAdminSettingsRow = await res.json()
          set((d) => {
            d.settings = row
            d.saving = false
          })
          emitMemoryAdminSettingsUpdated(row).catch(() => {})
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

export const MemoryAdminStoreProxy = createStoreProxy(useMemoryAdminStore)
