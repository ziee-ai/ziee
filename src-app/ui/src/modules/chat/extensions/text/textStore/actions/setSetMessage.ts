import type { TextStoreGet, TextStoreSet } from '../state'

/** Register setter function (called by TextInput on mount). */
export default (set: TextStoreSet, _get: TextStoreGet) =>
  (setter: (text: string) => void) => {
    set(state => {
      state.setMessage = setter
    })
  }
