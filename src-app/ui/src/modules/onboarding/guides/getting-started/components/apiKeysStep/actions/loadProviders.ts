import type { ApiKeysStepSet, ApiKeysStepGet } from '../state'
import loadProvidersFactory from './_loadProviders'

export default (set: ApiKeysStepSet, get: ApiKeysStepGet) => {
  const loadProviders = loadProvidersFactory(set, get)
  return async () => void loadProviders()
}
