import { create } from 'zustand'
import type {
  LlmModel,
  ModelCapabilities,
  ModelEngineSettings,
} from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { loadModelsForProvider } from './store'

/**
 * Upload progress for a single file
 */
export interface FileUploadProgress {
  filename: string
  progress: number // 0-100
  size: number // bytes
  status: 'pending' | 'uploading' | 'completed' | 'error'
}

/**
 * Upload request data
 */
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

/**
 * Upload store state
 */
interface UploadState {
  // Upload status
  uploading: boolean
  uploadProgress: FileUploadProgress[]
  overallUploadProgress: number
  uploadError: string | null
}

/**
 * Upload store with progress tracking
 */
export const useUploadStore = create<UploadState>()(() => ({
  uploading: false,
  uploadProgress: [],
  overallUploadProgress: 0,
  uploadError: null,
}))

/**
 * Upload local model with progress tracking using ApiClient
 */
export const uploadLocalModel = async (
  data: UploadModelData,
): Promise<LlmModel> => {
  try {
    // Initialize upload state
    useUploadStore.setState({
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

    // Create FormData
    const formData = new FormData()

    // Add files to FormData
    data.files.forEach(file => {
      formData.append('files', file)
    })

    // Add metadata fields
    formData.append('provider_id', data.provider_id)
    formData.append('name', data.name)
    formData.append('display_name', data.display_name)
    formData.append('main_filename', data.main_filename)
    formData.append('file_format', data.file_format)

    if (data.description) {
      formData.append('description', data.description)
    }

    if (data.capabilities) {
      formData.append('capabilities', JSON.stringify(data.capabilities))
    }

    if (data.engine_type) {
      formData.append('engine_type', data.engine_type)
    }

    if (data.engine_settings) {
      formData.append('engine_settings', JSON.stringify(data.engine_settings))
    }

    // Call the upload API with file upload progress tracking
    const model = await ApiClient.LlmModel.upload(formData as any, {
      fileUploadProgress: {
        onProgress: (
          progress: number,
          fileIndex: number,
          overallProgress: number,
        ) => {
          // Handle file-specific upload progress
          useUploadStore.setState(state => ({
            uploadProgress: state.uploadProgress.map((fp, index) =>
              index === fileIndex
                ? {
                    ...fp,
                    progress: Math.round(progress * 100),
                    status:
                      progress >= 1
                        ? ('completed' as const)
                        : ('uploading' as const),
                  }
                : fp,
            ),
            overallUploadProgress: Math.round(overallProgress * 100),
          }))
        },
        onComplete: () => {
          // Handle upload completion
          useUploadStore.setState(state => ({
            uploadProgress: state.uploadProgress.map(fp => ({
              ...fp,
              progress: 100,
              status: 'completed' as const,
            })),
            overallUploadProgress: 100,
            uploading: false,
          }))

          // Refresh the provider's models list (don't await to avoid blocking)
          loadModelsForProvider(data.provider_id)
        },
        onError: (error: string, fileName?: string) => {
          // Handle upload error
          useUploadStore.setState(state => ({
            uploadProgress: state.uploadProgress.map(fp =>
              fileName && fp.filename === fileName
                ? { ...fp, status: 'error' as const }
                : fp,
            ),
            uploadError: error || 'Upload failed',
            uploading: false,
          }))
        },
      },
    })

    return model
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : 'Failed to upload model'
    useUploadStore.setState({
      uploading: false,
      uploadError: errorMessage,
    })
    throw error
  }
}

/**
 * Clear upload progress and error state
 */
export const clearUploadState = () => {
  useUploadStore.setState({
    uploading: false,
    uploadProgress: [],
    overallUploadProgress: 0,
    uploadError: null,
  })
}

/**
 * Clear upload error only
 */
export const clearUploadError = () => {
  useUploadStore.setState({
    uploadError: null,
  })
}
