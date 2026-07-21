import type { MessageViewStateSet } from '../state'

export default (set: MessageViewStateSet) => {
  /** Show-more toggle for a long message (ITEM-4). */
  return (messageId: string, collapsed: boolean) =>
    set(d => {
      d.collapsed[messageId] = collapsed
    })
}
