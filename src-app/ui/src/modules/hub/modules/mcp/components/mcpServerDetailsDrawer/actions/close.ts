import type { McpServerDetailsDrawerSet } from '../state'

export default function closeFactory(set: McpServerDetailsDrawerSet) {
  return async () => {
    set({ isOpen: false, selectedServer: null, loading: false })
  }
}
