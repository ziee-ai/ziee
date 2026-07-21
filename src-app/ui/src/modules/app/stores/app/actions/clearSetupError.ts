import type { AppSet } from '../state'

export default (set: AppSet) => async () => {
  set({ setupError: null })
}
