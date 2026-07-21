import { ApiClient } from '@/api-client'
import type { RuntimeModelUsageGet, RuntimeModelUsageSet } from '../state'

export default (set: RuntimeModelUsageSet, _get: RuntimeModelUsageGet) =>
  async (modelId: string) => {
    try {
      const instance = await ApiClient.LocalRuntime.getInstance({ model_id: modelId })
      set(state => ({ instances: new Map(state.instances).set(modelId, instance) }))
    } catch {
      // 404 = no instance (never started / already reaped).
      set(state => ({ instances: new Map(state.instances).set(modelId, null) }))
    }
  }
