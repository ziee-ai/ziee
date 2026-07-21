import type { TextStoreGet, TextStoreSet } from '../state'

/** Clear text value via stored clearer. */
export default (_set: TextStoreSet, get: TextStoreGet) => () => {
  const { clearMessage } = get()
  if (!clearMessage) {
    console.warn('[TextStore] clearMessage function not registered')
    return
  }
  clearMessage()
}
