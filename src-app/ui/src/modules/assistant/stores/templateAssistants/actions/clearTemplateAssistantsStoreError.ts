import type { TemplateAssistantsSet } from '../state'

export default (set: TemplateAssistantsSet) => {
  (): void => {
    set({ error: null })
  }
}
