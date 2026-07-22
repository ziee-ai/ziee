import type { ModelPickerGet, ModelPickerSet } from '../state'
import firstEnabledModelIdFactory from './_firstEnabledModelId'
import { NEW_CHAT_MODEL_KEY } from '../state'

/**
 * The new-chat default model — for non-pane consumers (e.g. the workflow
 * run dialog) that just need a sensible current model.
 *
 * Action factories receive (set, get) in that order; this selector needs only
 * `get`, so `set` is the ignored first param (declaring `(get)` alone would bind
 * `get` to `set` → `get()` calls `set()` → undefined → crash).
 */
export default (_set: ModelPickerSet, get: ModelPickerGet) => {
  const firstEnabledModelId = firstEnabledModelIdFactory(get)
  return (): string | null =>
    get().selectedByConversation[NEW_CHAT_MODEL_KEY] ?? firstEnabledModelId()
}
