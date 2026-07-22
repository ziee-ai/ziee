import type { ModelPickerGet, ModelPickerSet } from '../state'

/** Get the selected model for a conversation key (null if unset).
 *  Action factories receive (set, get) in that order — this selector needs only
 *  `get`, so `set` is the ignored first param. Declaring `(get)` alone would bind
 *  `get` to `set`, so `get()` would call `set()` → undefined → a crash reading
 *  `selectedByConversation`. */
export default (_set: ModelPickerSet, get: ModelPickerGet) => {
  return (key: string): string | null =>
    get().selectedByConversation[key] ?? null
}
