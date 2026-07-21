import type {
  LlmProviderGroupWidgetGet,
  LlmProviderGroupWidgetSet,
} from '../state'
import loadFactory from './load'

/** Re-point this instance at a different group (defensive — parents should key
 *  widgets by group.id, but group.id can change in place). */
export default (
  set: LlmProviderGroupWidgetSet,
  get: LlmProviderGroupWidgetGet,
) => {
  const load = loadFactory(set, get)
  return async (groupId: string) => {
    if (get().groupId === groupId) return
    set(d => {
      d.groupId = groupId
    })
    void load(true)
  }
}
