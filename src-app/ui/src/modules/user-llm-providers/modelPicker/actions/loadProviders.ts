import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { sortProviders } from '@/modules/llm-provider/sortProviders'
import type { ModelPickerGet, ModelPickerSet } from '../state'
import initializeFromConversationFactory from './initializeFromConversation'
import { NEW_CHAT_MODEL_KEY } from '../state'

export default (set: ModelPickerSet, get: ModelPickerGet) => {
  const initializeFromConversation = initializeFromConversationFactory(set, get)
  return async () => {
    // Permission-gate the shell-eager-load fetch — the chat picker accesses
    // this on every chat render; the endpoint is gated on user_llm_providers::read.
    if (!hasPermissionNow(Permissions.UserLlmProvidersRead)) return
    set(state => {
      state.loading = true
      state.error = null
    })
    try {
      const response = await ApiClient.LlmProvider.getUserLlmProviders({}, undefined)
      set(state => {
        state.providers = sortProviders(response.providers)
        state.loading = false
      })
      // Seed the new-chat default if unset yet.
      if (!get().selectedByConversation[NEW_CHAT_MODEL_KEY]) {
        initializeFromConversation(NEW_CHAT_MODEL_KEY)
      }
    } catch (error: any) {
      console.error('[ModelPicker] loadProviders error:', error)
      set(state => {
        state.error = error.message || 'Failed to load providers'
        state.loading = false
      })
    }
  }
}
