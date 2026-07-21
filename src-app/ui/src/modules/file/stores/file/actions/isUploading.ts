import type { FileGet, FileSet } from '../state'
import { ownsId } from '../../composerOwnership'

/** Check if any of ONE pane's files are currently uploading. */
export default (_set: FileSet, get: FileGet) => (paneKey: string): boolean => {
  const owner = get().uploadOwner
  return Array.from(get().uploadingFiles.entries()).some(
    ([id, file]) =>
      ownsId(owner, id, paneKey) &&
      (file.status === 'pending' || file.status === 'uploading'),
  )
}
