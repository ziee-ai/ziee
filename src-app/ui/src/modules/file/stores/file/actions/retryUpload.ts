import type { FileGet, FileSet } from '../state'
import removeUploadingFileFactory from './removeUploadingFile'
import uploadFilesFactory from './uploadFiles'

/** Retry a failed upload: drop the errored entry and re-run the upload for
 *  its retained raw File, producing a fresh progress entry (into the same
 *  pane's buffer). No-op if the entry is missing or the raw File wasn't retained. */
export default (set: FileSet, get: FileGet) => async (paneKey: string, progressId: string) => {
  const entry = get().uploadingFiles.get(progressId)
  if (!entry?.rawFile) return
  const removeUploadingFile = removeUploadingFileFactory(set, get)
  const uploadFiles = uploadFilesFactory(set, get)
  removeUploadingFile(progressId)
  await uploadFiles(paneKey, [entry.rawFile])
}
