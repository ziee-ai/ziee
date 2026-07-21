import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { templateAssistantsState } from './state'

const TemplateAssistantsDef = defineStore('TemplateAssistants', {
  immer: true,
  state: templateAssistantsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadTemplateAssistants()
    on('assistant_template.created', reload)
    on('assistant_template.updated', reload)
    on('assistant_template.deleted', reload)
    // The load action self-gates on AssistantsTemplateRead.
    on('sync:assistant_template', reload)
    on('sync:reconnect', reload)
    void actions.loadTemplateAssistants()
  },
})

export const TemplateAssistants = registerLazyStore(TemplateAssistantsDef)
export const useTemplateAssistantsStore = TemplateAssistantsDef.store

// Raw store for direct access (Stores proxy uses this).
export { TemplateAssistantsDef }
