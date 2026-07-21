import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { summarizationAdminState, type SummarizationAdminState } from './state'
import type { Actions } from './actions.gen'

const SummarizationAdminDef = defineStore<SummarizationAdminState, Actions>('SummarizationAdmin', {
  immer: true,
  state: summarizationAdminState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Singleton row; sync entity id is nil. Self-gate on
    // summarization::settings::read to skip refetches for non-admins (the
    // chat-extension pill reads from this store on every conversation switch).
    const reload = () => {
      if (!hasPermissionNow(Permissions.SummarizationSettingsRead)) return
      void actions.load()
    }
    on('sync:summarization_admin_settings', reload)
    on('sync:reconnect', reload)
    if (hasPermissionNow(Permissions.SummarizationSettingsRead)) {
      void actions.load()
      void actions.loadAvailableModels()
    }
  },
})
export const SummarizationAdmin = registerLazyStore(SummarizationAdminDef)
export const useSummarizationAdminStore = SummarizationAdminDef.store
