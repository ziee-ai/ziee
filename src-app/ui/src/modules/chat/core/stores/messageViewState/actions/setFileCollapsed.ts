import type { MessageViewStateSet } from '../state'
import ensureFileFactory from './_ensureFile'

export default (set: MessageViewStateSet) => {
  const ensureFile = ensureFileFactory(set)
  /** Inline-file chevron toggle (ITEM-5). */
  return (key: string, collapsed: boolean) =>
    set(d => {
      ensureFile(d.files, key).collapsed = collapsed
    })
}
