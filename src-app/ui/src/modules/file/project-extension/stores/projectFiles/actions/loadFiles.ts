import type { ProjectFilesGet, ProjectFilesSet } from '../state'
import loadFilesFactory from './_loadFiles'

export default (set: ProjectFilesSet, get: ProjectFilesGet) => {
  const _loadFiles = loadFilesFactory(set, get)
  return async (projectId: string) => _loadFiles(projectId)
}
