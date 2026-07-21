import type { TextStoreGet, TextStoreSet } from '../state'

/** Get current text value via stored getter. */
export default (_set: TextStoreSet, get: TextStoreGet) => (): string => {
  const { getMessage } = get()
  if (!getMessage) {
    console.warn('[TextStore] getMessage function not registered')
    return ''
  }
  return getMessage()
}
