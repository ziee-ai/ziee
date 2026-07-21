import type { AssistantPickerSet } from '../state'

export default (set: AssistantPickerSet) => (key: string) => {
  set(s => {
    s.selectedByConversation[key] = null
  })
}
