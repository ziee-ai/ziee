import type { StoreSet } from '@ziee/framework/store-kit'

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
// Exported via a getter/setter so actions can wire/unset the XHR.
let _currentUploadXhr: XMLHttpRequest | null = null

export function getCurrentUploadXhr(): XMLHttpRequest | null {
  return _currentUploadXhr
}

export function setCurrentUploadXhr(xhr: XMLHttpRequest | null): void {
  _currentUploadXhr = xhr
}

export const voiceModelUploadState = {
  uploading: false,
  uploadProgress: [] as FileUploadProgress[],
  overallUploadProgress: 0,
  uploadError: null as string | null,
}

export type VoiceModelUploadState = typeof voiceModelUploadState
export type VoiceModelUploadSet = StoreSet<VoiceModelUploadState>
export type VoiceModelUploadGet = () => VoiceModelUploadState
