import type { MessageViewStateSet } from '../state'
import ensureFileFactory from './_ensureFile'

export default (set: MessageViewStateSet) => {
  const ensureFile = ensureFileFactory(set)
  /** Mark an inline file's body as having entered view once (ITEM-5). */
  return (key: string) =>
    set(d => {
      ensureFile(d.files, key).seen = true
    })
}
