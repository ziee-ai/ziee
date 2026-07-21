import type { MemoriesGet, MemoriesSet } from '../state'
import loadFactory from './load'

// Debounce timer for search-query reloads — keystrokes within 250ms coalesce.
let searchDebounce: ReturnType<typeof setTimeout> | null = null

export default (set: MemoriesSet, get: MemoriesGet) => {
  const load = loadFactory(set, get)
  return async (q: string) => {
    set(s => {
      s.searchQuery = q
      s.currentPage = 1
    })
    if (searchDebounce) clearTimeout(searchDebounce)
    searchDebounce = setTimeout(() => {
      void load(1)
    }, 250)
  }
}
