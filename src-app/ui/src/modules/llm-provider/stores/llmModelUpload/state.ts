import type { StoreSet } from '@ziee/framework/store-kit'

/** Upload progress for a single file */
export interface FileUploadProgress {
  filename: string
  progress: number // 0-100
  size: number // bytes
  status: 'pending' | 'uploading' | 'completed' | 'error'
}

export const llmModelUploadState = {
  uploading: false,
  uploadProgress: [] as FileUploadProgress[],
  overallUploadProgress: 0,
  uploadError: null as string | null,
}

export type LlmModelUploadState = typeof llmModelUploadState
export type LlmModelSet = StoreSet<LlmModelUploadState>
export type LlmModelGet = () => LlmModelUploadState
