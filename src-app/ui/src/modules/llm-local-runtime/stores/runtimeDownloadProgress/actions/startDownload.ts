import { ApiClient } from '@/api-client'
import subscribeToKeyFactory from '../subscribeToKey'
import type { RuntimeDownloadProgressGet, RuntimeDownloadProgressSet } from '../state'
import type { RuntimeDownloadRequest } from '@/modules/llm-local-runtime/types'

export default (set: RuntimeDownloadProgressSet, _get: RuntimeDownloadProgressGet) => {
  const subscribeToKey = subscribeToKeyFactory(set, (key: string) => {
    set(state => {
      const next = new Map(state.activeByKey)
      next.delete(key)
      return { activeByKey: next }
    })
  })
  return async (req: RuntimeDownloadRequest): Promise<{ key: string }> => {
    const started = await ApiClient.RuntimeVersion.download(req)
    const key = started.key
    // Seed an in-progress entry immediately so the UI repaints before the
    // first SSE chunk lands.
    set(state => {
      const next = new Map(state.activeByKey)
      next.set(key, {
        task_id: started.task_id,
        key,
        engine: started.engine,
        version: started.version,
        backend: started.backend,
        status: started.status,
        bytes_received: 0,
      })
      return { activeByKey: next }
    })
    subscribeToKey(key)
    return { key }
  }
}
