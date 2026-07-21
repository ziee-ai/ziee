import type { TextStoreGet, TextStoreSet } from '../state'

/** Set backup message (before clearing). */
export default (set: TextStoreSet, _get: TextStoreGet) => (text: string | null) => {
  set(state => {
    state.backupMessage = text
  })
}
