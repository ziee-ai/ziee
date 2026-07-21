import type { FileGet, FileSet } from '../state'
import { ownsId } from '../../composerOwnership'

/** Get array of file entities for ONE pane's buffer (safe outside React). */
export default (_set: FileSet, get: FileGet) => (paneKey: string) => {
  const owner = get().fileOwner
  return Array.from(get().selectedFiles.entries())
    .filter(([id]) => ownsId(owner, id, paneKey))
    .map(([, file]) => file)
}
