import type { ModelPickerGet } from '../state'
import firstEnabledModelIdFactory from './_firstEnabledModelId'
import { NEW_CHAT_MODEL_KEY } from '../state'

/**
 * The new-chat default model — for non-pane consumers (e.g. the workflow
 * run dialog) that just need a sensible current model.
 */
export default (get: ModelPickerGet) => {
  const firstEnabledModelId = firstEnabledModelIdFactory(get)
  return (): string | null =>
    get().selectedByConversation[NEW_CHAT_MODEL_KEY] ?? firstEnabledModelId()
}
