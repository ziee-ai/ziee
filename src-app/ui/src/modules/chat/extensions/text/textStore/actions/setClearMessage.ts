import type { TextStoreGet, TextStoreSet } from '../state'

/** Register clear function (called by TextInput on mount). */
export default (set: TextStoreSet, _get: TextStoreGet) => (clearer: () => void) => {
  set(state => {
    state.clearMessage = clearer
  })
}
