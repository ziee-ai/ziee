import type { ModelDetailsDrawerSet } from '../state'

export default (set: ModelDetailsDrawerSet) => () => {
  set({ isOpen: false, selectedModel: null, loading: false })
}
