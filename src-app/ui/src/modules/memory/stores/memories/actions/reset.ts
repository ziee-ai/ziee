import type { MemoriesSet } from '../state'

export default (set: MemoriesSet) => async () => {
  set(s => {
    s.memories = []
    s.loading = false
    s.saving = false
    s.error = null
    s.searchQuery = ''
    s.kindFilter = null
    s.sourceFilter = null
  })
}
