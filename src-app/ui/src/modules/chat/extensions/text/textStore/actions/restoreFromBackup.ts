import type { TextStoreGet, TextStoreSet } from '../state'

/** Restore text from backup. */
export default (_set: TextStoreSet, get: TextStoreGet) => () => {
  const { backupMessage, setMessage } = get()
  if (backupMessage && setMessage) {
    setMessage(backupMessage)
    console.log('[TextStore] Restored text from backup')
  }
}
