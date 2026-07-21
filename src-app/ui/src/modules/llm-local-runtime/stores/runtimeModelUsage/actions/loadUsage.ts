import { ApiClient } from '@/api-client'
import type { RuntimeEngine } from '../../../types'
import type { RuntimeModelUsageGet, RuntimeModelUsageSet } from '../state'

export default (set: RuntimeModelUsageSet, _get: RuntimeModelUsageGet) =>
  async (engine: RuntimeEngine) => {
    set(state => ({
      loading: new Map(state.loading).set(engine, true),
      error: null,
    }))
    try {
      const response = await ApiClient.RuntimeVersion.usage({ engine })
      set(state => {
        const loading = new Map(state.loading)
        loading.delete(engine)
        return { loading, usage: new Map(state.usage).set(engine, response) }
      })
    } catch (error) {
      set(state => {
        const loading = new Map(state.loading)
        loading.delete(engine)
        return {
          loading,
          error: error instanceof Error ? error.message : 'Failed to load usage',
        }
      })
    }
  }
