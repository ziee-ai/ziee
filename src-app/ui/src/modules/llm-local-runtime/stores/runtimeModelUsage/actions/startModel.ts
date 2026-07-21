import { ApiClient } from '@/api-client'
import type { RuntimeEngine } from '../../../types'
import { emitRuntimeModelUsageChanged } from '../../../events/emitters'
import actFactory from './_act'
import loadUsageFactory from './loadUsage'
import type { RuntimeModelUsageGet, RuntimeModelUsageSet } from '../state'

export default (set: RuntimeModelUsageSet, get: RuntimeModelUsageGet) => {
  const act = actFactory(set)
  const loadUsage = loadUsageFactory(set, get)
  return async (engine: RuntimeEngine, modelId: string) => {
    await act(modelId, () => ApiClient.LocalRuntime.startModel({ model_id: modelId }))
    await loadUsage(engine)
    await emitRuntimeModelUsageChanged(modelId)
  }
}
