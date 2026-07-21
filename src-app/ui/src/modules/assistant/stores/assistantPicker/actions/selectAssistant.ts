import type { AssistantPickerSet } from '../state'

export default (set: AssistantPickerSet) =>
  (key: string, assistantId: string) => {
    set(s => {
      s.selectedByConversation[key] = assistantId
    })
  }
