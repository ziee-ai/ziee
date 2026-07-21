import doLoadModelsFactory from './_doLoadModels'
import type { SummarizationAdminGet, SummarizationAdminSet } from '../state'

export default (set: SummarizationAdminSet, get: SummarizationAdminGet) => {
  const doLoadModels = doLoadModelsFactory(set, get)
  return async () => doLoadModels()
}
