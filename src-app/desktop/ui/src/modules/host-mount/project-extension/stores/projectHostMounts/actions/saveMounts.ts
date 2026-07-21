import type { MountEntry } from '@/api-client/types'
import type { ProjectHostMountsGet, ProjectHostMountsSet } from '../state'
import doSaveFactory from './_doSave'

export default (set: ProjectHostMountsSet, _get: ProjectHostMountsGet) => {
  const doSave = doSaveFactory(set)
  return async (projectId: string, mounts: MountEntry[]) => doSave(projectId, mounts)
}
