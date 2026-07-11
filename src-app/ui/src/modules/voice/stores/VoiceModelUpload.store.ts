import { ApiClient } from '@/api-client'
import type { VoiceModel } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'
import { Stores } from '@/core/stores'

/** Upload progress for a single file. */
export interface FileUploadProgress {
  filename: string
  progress: number // 0-100
  size: number // bytes
  status: 'pending' | 'uploading' | 'completed' | 'error'
}

/** Upload request data — one .bin/.gguf ggml model file + a display name. */
export interface UploadVoiceModelData {
  name: string
  file: File
}

// Store XHR for cancellation (module-scope: not serializable / reactive).
let currentUploadXhr: XMLHttpRequest | null = null

/**
 * Whisper-model upload store with progress tracking. Mirrors
 * llm-provider's `LlmModelUpload` (XHR FormData with per-file + overall
 * progress), single-file.
 */
export const VoiceModelUpload = defineStore('VoiceModelUpload', {
  state: {
    uploading: false,
    uploadProgress: [] as FileUploadProgress[],
    overallUploadProgress: 0,
    uploadError: null as string | null,
  },
  actions: set => ({
    uploadModel: async (data: UploadVoiceModelData): Promise<VoiceModel> => {
      try {
        set({
          uploading: true,
          uploadError: null,
          uploadProgress: [
            {
              filename: data.file.name,
              progress: 0,
              size: data.file.size,
              status: 'pending' as const,
            },
          ],
          overallUploadProgress: 0,
        })

        const formData = new FormData()
        formData.append('file', data.file)
        formData.append('name', data.name)

        const model = await ApiClient.Voice.uploadModel(formData as any, {
          fileUploadProgress: {
            __init: (xhr: XMLHttpRequest) => {
              currentUploadXhr = xhr
            },
            onProgress: (
              progress: number,
              fileIndex: number,
              overallProgress: number,
            ) => {
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
              // Refresh the installed-models library (don't await to avoid blocking).
              void Stores.VoiceModel.loadInstalled()
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
      set({
        uploading: false,
        uploadProgress: [],
        overallUploadProgress: 0,
        uploadError: null,
      })
    },
    clearUploadError: () => {
      set({ uploadError: null })
    },
  }),
})

export const useVoiceModelUploadStore = VoiceModelUpload.store
