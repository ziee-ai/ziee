import type { TextStoreGet, TextStoreSet } from '../state'

/** Get backup message. */
export default (_set: TextStoreSet, get: TextStoreGet) => (): string | null =>
  get().backupMessage
