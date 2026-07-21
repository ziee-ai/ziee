import type { ModelPickerSet } from '../state'

/** Set the selected model for one conversation key. */
export default (set: ModelPickerSet) => {
  return (key: string, id: string) => {
    set(state => {
      state.selectedByConversation[key] = id
    })
  }
}
