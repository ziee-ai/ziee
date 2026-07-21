import type { MemoriesGet, MemoriesSet } from '../state'
import loadFactory from './load'

export default (set: MemoriesSet, get: MemoriesGet) => {
  const load = loadFactory(set, get)
  return async (k: string | null) => {
    set(s => {
      s.kindFilter = k
      s.currentPage = 1
    })
    void load(1)
  }
}
