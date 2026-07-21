import type { LlmModel, ModelCapabilities, ModelEngineSettings } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import type { LlmModelGet, LlmModelSet } from '../state'

// Module-scope XHR for cancellation (not serializable / reactive).
let _xhr: XMLHttpRequest | null = null

/**
 * Exposed only so cancelUpload can read the current XHR reference.
 * Do NOT call from outside this module.
 */
export function getCurrentXhr(): XMLHttpRequest | null {
  return _xhr
}

/**
 * Core upload implementation.
 * Imported as a factory from `uploadLocalModel.ts` so the action wrapper
 * can pass (set, get) and the drawer component can reach `useLlmProviderStore`.
 */
export default (set: LlmModelSet, _get: LlmModelGet) =>
  async (
    providerId: string,
    name: string,
    displayName: string,
    description: string | undefined,
    mainFilename: string,
    fileFormat: string,
    capabilities: ModelCapabilities | undefined,
    engineType: string | undefined,
    engineSettings: ModelEngineSettings | undefined,
    files: File[],
  ): Promise<LlmModel> => {
    const { useLlmProviderStore } = await import('@/modules/llm-provider/stores/llmProvider')

    try {
      set(s => {
        s.uploading = true
        s.uploadError = null
        s.uploadProgress = files.map(file => ({
          filename: file.name,
          progress: 0,
          size: file.size,
          status: 'pending' as const,
        }))
        s.overallUploadProgress = 0
      })

      const formData = new FormData()
      files.forEach(file => {
        formData.append('files', file)
      })
      formData.append('provider_id', providerId)
      formData.append('name', name)
      formData.append('display_name', displayName)
      formData.append('main_filename', mainFilename)
      formData.append('file_format', fileFormat)
      if (description) formData.append('description', description)
      if (capabilities) formData.append('capabilities', JSON.stringify(capabilities))
      if (engineType) formData.append('engine_type', engineType)
      if (engineSettings) formData.append('engine_settings', JSON.stringify(engineSettings))

      const model = await ApiClient.LlmModel.upload(formData as any, {
        fileUploadProgress: {
          __init: (xhr: XMLHttpRequest) => {
            _xhr = xhr
          },
          onProgress: (progress: number, fileIndex: number, overallProgress: number) => {
            set(s => ({
              uploadProgress: s.uploadProgress.map((fp, index) =>
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
            set(s => {
              s.uploadProgress = s.uploadProgress.map(fp => ({
                ...fp,
                progress: 100,
                status: 'completed' as const,
              }))
              s.overallUploadProgress = 100
              s.uploading = false
            })
            _xhr = null
            // Refresh the provider's models list (don't await to avoid blocking).
            void useLlmProviderStore.getState().loadModelsForProvider(providerId)
          },
          onError: (error: string, fileName?: string) => {
            set(s => {
              s.uploadProgress = s.uploadProgress.map(fp =>
                fileName && fp.filename === fileName
                  ? { ...fp, status: 'error' as const }
                  : fp,
              )
              s.uploadError = error || 'Upload failed'
              s.uploading = false
            })
            _xhr = null
          },
        },
      })
      return model
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Failed to upload model'
      set({ uploading: false, uploadError: errorMessage })
      _xhr = null
      throw error
    }
  }
