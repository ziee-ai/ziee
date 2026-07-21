import type { FileGet, FileSet } from '../state'
import { ownedIds } from '../../composerOwnership'

/** Get array of file IDs for request composition (ONE pane's buffer, ITEM-32). */
export default (_set: FileSet, get: FileGet) => (paneKey: string): string[] => {
  return ownedIds(get().selectedFiles.keys(), get().fileOwner, paneKey)
}
