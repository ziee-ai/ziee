import type { StoreSet } from '@ziee/framework/store-kit'
import type { File as ProjectFile } from '@/api-client/types'

/**
 * Per-file upload progress. The map key is a synthetic local id (so the same
 * file uploaded twice gets two separate progress rows).
 */
export interface ProjectFileUploadProgress {
  id: string
  filename: string
  size: number
  progress: number
  status: 'pending' | 'uploading' | 'error'
  error?: string
}

export const projectFilesState = {
  /** Active project id, mirrored from `ProjectDetail.project.id`. */
  currentProjectId: null as string | null,
  files: [] as ProjectFile[],
  filesLoading: false,
  uploadingFiles: new Map<string, ProjectFileUploadProgress>(),
  selectedFileIds: new Set<string>(),
  attaching: false,
  detaching: false,
  error: null as string | null,
}

export type ProjectFilesState = typeof projectFilesState
export type ProjectFilesSet = StoreSet<ProjectFilesState>
export type ProjectFilesGet = () => ProjectFilesState
