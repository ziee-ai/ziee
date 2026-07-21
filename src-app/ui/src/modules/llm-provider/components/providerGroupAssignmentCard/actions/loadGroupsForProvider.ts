import { ApiClient } from '@/api-client'
import type { ProviderGroupAssignmentCardGet, ProviderGroupAssignmentCardSet } from '../state'

export default (set: ProviderGroupAssignmentCardSet, get: ProviderGroupAssignmentCardGet) =>
  async (providerId: string, force = false): Promise<void> => {
    const state = get()
    const existing = state.providerGroups.get(providerId)
    if (existing?.loading && !force) return
    // Fresh (< 30s) and not forcing → cached.
    if (
      !force &&
      existing?.lastFetched &&
      Date.now() - existing.lastFetched < 30000 &&
      !existing.error
    ) {
      return
    }
    set(s => {
      s.providerGroups.set(providerId, {
        providerId,
        groups: existing?.groups || [],
        loading: true,
        error: null,
        lastFetched: existing?.lastFetched || null,
      })
    })
    try {
      const groups = await ApiClient.LlmProvider.getGroups({ provider_id: providerId })
      set(s => {
        s.providerGroups.set(providerId, {
          providerId,
          groups,
          loading: false,
          error: null,
          lastFetched: Date.now(),
        })
      })
    } catch (error) {
      console.error(`Failed to load groups for provider ${providerId}:`, error)
      set(s => {
        s.providerGroups.set(providerId, {
          providerId,
          groups: existing?.groups || [],
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to load groups',
          lastFetched: existing?.lastFetched || null,
        })
      })
      throw error
    }
  }
