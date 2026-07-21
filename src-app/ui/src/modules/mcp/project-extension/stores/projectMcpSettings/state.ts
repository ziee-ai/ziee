import type { ProjectMcpSettingsResponse } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

/** Canonical in-store representation (the GET-shape response). */
export type ProjectMcpSettings = ProjectMcpSettingsResponse

export const projectMcpSettingsState = {
  currentProjectId: null as string | null,
  settings: null as ProjectMcpSettings | null,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type ProjectMcpSettingsState = typeof projectMcpSettingsState
export type ProjectMcpSettingsSet = StoreSet<ProjectMcpSettingsState>
export type ProjectMcpSettingsGet = () => ProjectMcpSettingsState
