import type { ProjectHostMountsGet, ProjectHostMountsSet } from '../state'
import doLoadFactory from './_doLoad'

export default (set: ProjectHostMountsSet, _get: ProjectHostMountsGet) => {
  const doLoad = doLoadFactory(set)
  return async (projectId: string) => doLoad(projectId)
}
