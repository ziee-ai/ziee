import type { ModelPickerGet } from '../state'

/** Get the selected model for a conversation key (null if unset). */
export default (get: ModelPickerGet) => {
  return (key: string): string | null =>
    get().selectedByConversation[key] ?? null
}
