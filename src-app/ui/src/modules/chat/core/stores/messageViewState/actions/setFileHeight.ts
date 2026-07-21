import type { MessageViewStateSet } from '../state'
import ensureFileFactory from './_ensureFile'

export default (set: MessageViewStateSet) => {
  const ensureFile = ensureFileFactory(set)
  /** Persist a user-dragged inline-file body height in px (ITEM-3/5). */
  return (key: string, heightPx: number) =>
    set(d => {
      ensureFile(d.files, key).heightPx = heightPx
    })
}
