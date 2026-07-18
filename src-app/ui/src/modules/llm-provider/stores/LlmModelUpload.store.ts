import type {
  LlmModel,
  ModelCapabilities,
  ModelEngineSettings,
} from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { defineStore } from '@ziee/framework/store-kit'
import { useLlmProviderStore } from '@/modules/llm-provider/stores/LlmProvider.store'

/** Upload progress for a single file */
export interface FileUploadProgress {
  filename: string
  progress: number // 0-100
  size: number // bytes
  status: 'pending' | 'uploading' | 'completed' | 'error'
}

/** Upload request data */
export interface UploadModelData {
  name: string // auto-generated model ID
  provider_id: string
  display_name: string
  description?: string
  main_filename: string
  file_format: string
  capabilities?: ModelCapabilities
  engine_type?: string
  engine_settings?: ModelEngineSettings
  files: File[]
}

// Store XHR for cancellation (module-scope: not serializable / reactive).
let currentUploadXhr: XMLHttpRequest | null = null

/** Upload store with progress tracking */
export const LlmModelUpload = defineStore('LlmModelUpload', {
  state: {
    uploading: false,
    uploadProgress: [] as FileUploadProgress[],
    overallUploadProgress: 0,
    uploadError: null as string | null,
  },
  actions: set => ({
    uploadLocalModel: async (data: UploadModelData): Promise<LlmModel> => {
      try {
        set({
          uploading: true,
          uploadError: null,
          uploadProgress: data.files.map(file => ({
            filename: file.name,
            progress: 0,
            size: file.size,
            status: 'pending' as const,
          })),
          overallUploadProgress: 0,
        })

        const formData = new FormData()
        data.files.forEach(file => {
          formData.append('files', file)
        })
        formData.append('provider_id', data.provider_id)
        formData.append('name', data.name)
        formData.append('display_name', data.display_name)
        formData.append('main_filename', data.main_filename)
        formData.append('file_format', data.file_format)
        if (data.description) formData.append('description', data.description)
        if (data.capabilities)
          formData.append('capabilities', JSON.stringify(data.capabilities))
        if (data.engine_type) formData.append('engine_type', data.engine_type)
        if (data.engine_settings)
          formData.append('engine_settings', JSON.stringify(data.engine_settings))

        const model = await ApiClient.LlmModel.upload(formData as any, {
          fileUploadProgress: {
            __init: (xhr: XMLHttpRequest) => {
              currentUploadXhr = xhr
            },
            onProgress: (progress: number, fileIndex: number, overallProgress: number) => {
              // progress/overallProgress already in 0-100 range from core.ts
              set(state => ({
                uploadProgress: state.uploadProgress.map((fp, index) =>
                  index === fileIndex
                    ? {
                        ...fp,
                        progress: Math.round(progress),
                        status:
                          progress >= 100
                            ? ('completed' as const)
                            : ('uploading' as const),
                      }
                    : fp,
                ),
                overallUploadProgress: Math.round(overallProgress),
              }))
            },
            onComplete: () => {
              set(state => ({
                uploadProgress: state.uploadProgress.map(fp => ({
                  ...fp,
                  progress: 100,
                  status: 'completed' as const,
                })),
                overallUploadProgress: 100,
                uploading: false,
              }))
              currentUploadXhr = null
              // Refresh the provider's models list (don't await to avoid blocking).
              void useLlmProviderStore.getState().loadModelsForProvider(data.provider_id)
            },
            onError: (error: string, fileName?: string) => {
              set(state => ({
                uploadProgress: state.uploadProgress.map(fp =>
                  fileName && fp.filename === fileName
                    ? { ...fp, status: 'error' as const }
                    : fp,
                ),
                uploadError: error || 'Upload failed',
                uploading: false,
              }))
              currentUploadXhr = null
            },
          },
        })
        return model
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Failed to upload model'
        set({ uploading: false, uploadError: errorMessage })
        currentUploadXhr = null
        throw error
      }
    },
    cancelUpload: () => {
      if (currentUploadXhr) {
        currentUploadXhr.abort()
        currentUploadXhr = null
      }
    },
    clearUploadState: () => {
      set({ uploading: false, uploadProgress: [], overallUploadProgress: 0, uploadError: null })
    },
    clearUploadError: () => {
      set({ uploadError: null })
    },
  }),
})

export const useUploadStore = LlmModelUpload.store
