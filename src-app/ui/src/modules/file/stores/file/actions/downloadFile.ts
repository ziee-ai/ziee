import { ApiClient } from '@/api-client'
import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Triggers a browser download for the given file. Throws on failure. */
export default (_set: FileSet, _get: FileGet) => async (file: FileEntity) => {
  const response = await ApiClient.File.download({ file_id: file.id })
  const blob = response instanceof Blob ? response : new Blob([response])
  const url = window.URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = file.filename
  document.body.appendChild(a)
  a.click()
  window.URL.revokeObjectURL(url)
  document.body.removeChild(a)
}
