import type { TextStoreGet, TextStoreSet } from '../state'

/** Register getter function (called by TextInput on mount). */
export default (set: TextStoreSet, _get: TextStoreGet) => (getter: () => string) => {
  set(state => {
    state.getMessage = getter
  })
}
