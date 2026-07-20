import type { MemoryAuditGet, MemoryAuditSet } from '../state'
import doLoadFactory from './_doLoad'

export default (set: MemoryAuditSet, get: MemoryAuditGet) => {
  const doLoad = doLoadFactory(set, get)
  return async (limit: number) => {
    set(s => {
      s.limit = limit
    })
    void doLoad(limit)
  }
}
