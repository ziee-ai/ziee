import type { AssistantPickerGet, AssistantPickerSet } from '../state'

export default (
  _set: AssistantPickerSet,
  get: AssistantPickerGet,
) => (key: string): string | null => get().selectedByConversation[key] ?? null
