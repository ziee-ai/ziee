import type { MemoryAuditGet, MemoryAuditSet } from '../state'
import doLoadFactory from './_doLoad'

export default (set: MemoryAuditSet, get: MemoryAuditGet) => {
  const doLoad = doLoadFactory(set, get)
  return async (limit?: number) => doLoad(limit ?? get().limit)
}
