import { ApiClient } from '@/api-client'
import type { VoiceModel } from '@/api-client/types'
import type { VoiceModelUploadSet, VoiceModelUploadGet } from '../state'
import { setCurrentUploadXhr } from '../state'
import { VoiceModel as VoiceModelStore } from '@/modules/voice/stores/voiceModel'

export default (set: VoiceModelUploadSet, _get: VoiceModelUploadGet) =>
  async (data: { name: string; file: File }): Promise<VoiceModel> => {
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
            setCurrentUploadXhr(xhr)
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
            setCurrentUploadXhr(null)
            // Refresh the installed-models library (don't await to avoid blocking).
            void VoiceModelStore.loadInstalled()
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
            setCurrentUploadXhr(null)
          },
        },
      })
      return model
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Failed to upload model'
      set({ uploading: false, uploadError: errorMessage })
      setCurrentUploadXhr(null)
      throw error
    }
  }
