import doLoadFactory from './_doLoad'
import type { SummarizationAdminGet, SummarizationAdminSet } from '../state'

export default (set: SummarizationAdminSet, get: SummarizationAdminGet) => {
  const doLoad = doLoadFactory(set, get)
  return async () => doLoad()
}
