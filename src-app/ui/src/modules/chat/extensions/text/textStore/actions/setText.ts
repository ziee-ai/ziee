import type { TextStoreGet, TextStoreSet } from '../state'

/** Set text value via stored setter. */
export default (_set: TextStoreSet, get: TextStoreGet) => (text: string) => {
  const { setMessage } = get()
  if (!setMessage) {
    console.warn('[TextStore] setMessage function not registered')
    return
  }
  setMessage(text)
}
