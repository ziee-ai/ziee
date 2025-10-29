import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  LlmProvider,
  CreateLlmProviderRequest,
  UpdateLlmProviderRequest,
} from '@/api-client/types'

interface LlmProviderState {
  // Data
  providers: LlmProvider[]
  isInitialized: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Error state
  error: string | null

  __init__: {
    providers: () => Promise<void>
  }
}

export const useLlmProviderStore = create<LlmProviderState>()(
  subscribeWithSelector(
    (): LlmProviderState => ({
      // Initial state
      providers: [],
      isInitialized: false,
      loading: false,
      creating: false,
      updating: false,
      deleting: false,
      error: null,
      __init__: {
        providers: async () => loadLlmProviders(),
      },
    }),
  ),
)

// Provider actions
export const loadLlmProviders = async (): Promise<void> => {
  const state = useLlmProviderStore.getState()
  if (state.isInitialized || state.loading) {
    return
  }
  try {
    useLlmProviderStore.setState({ loading: true, error: null })

    const response = await ApiClient.LlmProvider.list({
      page: 1,
      per_page: 50,
    })

    useLlmProviderStore.setState({
      providers: response.providers,
      isInitialized: true,
      loading: false,
    })
  } catch (error) {
    useLlmProviderStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to load providers',
      loading: false,
    })
    throw error
  }
}

export const createLlmProvider = async (
  data: CreateLlmProviderRequest,
): Promise<LlmProvider> => {
  const state = useLlmProviderStore.getState()
  if (state.creating) {
    return Promise.resolve(null as any)
  }

  try {
    useLlmProviderStore.setState({ creating: true, error: null })

    const provider = await ApiClient.LlmProvider.create(data)

    useLlmProviderStore.setState(state => ({
      providers: [...state.providers, provider],
      creating: false,
    }))

    return provider
  } catch (error) {
    useLlmProviderStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to create provider',
      creating: false,
    })
    throw error
  }
}

export const updateLlmProvider = async (
  id: string,
  data: UpdateLlmProviderRequest,
): Promise<LlmProvider> => {
  const state = useLlmProviderStore.getState()
  if (state.updating) {
    return Promise.resolve(null as any)
  }

  try {
    useLlmProviderStore.setState({ updating: true, error: null })

    const provider = await ApiClient.LlmProvider.update({
      provider_id: id,
      ...data,
    })

    useLlmProviderStore.setState(state => ({
      providers: state.providers.map(p => (p.id === id ? provider : p)),
      updating: false,
    }))

    return provider
  } catch (error) {
    useLlmProviderStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to update provider',
      updating: false,
    })
    throw error
  }
}

export const deleteLlmProvider = async (id: string): Promise<void> => {
  const state = useLlmProviderStore.getState()
  if (state.deleting) {
    return
  }

  try {
    useLlmProviderStore.setState({ deleting: true, error: null })

    await ApiClient.LlmProvider.delete({ provider_id: id })

    useLlmProviderStore.setState(state => ({
      providers: state.providers.filter(p => p.id !== id),
      deleting: false,
    }))
  } catch (error) {
    useLlmProviderStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to delete provider',
      deleting: false,
    })
    throw error
  }
}

export const clearLlmProviderStoreError = (): void => {
  useLlmProviderStore.setState({ error: null })
}

export const findLlmProviderById = (id: string): LlmProvider | undefined => {
  return useLlmProviderStore
    .getState()
    .providers.find(p => p.id === id)
}

export const llmProviderHasCredentials = (
  provider: LlmProvider,
): boolean => {
  // Local providers don't need credentials
  if (provider.provider_type === 'local') {
    return true
  }

  // Custom providers might not require API keys
  if (provider.provider_type === 'custom') {
    return true
  }

  // Check if API key exists and is not empty
  return !!(provider.api_key && provider.api_key.trim())
}

// Re-export drawer store functions and hook
export { useLlmProviderDrawerStore, openLlmProviderDrawer, closeLlmProviderDrawer } from './drawer-store'

// Re-export for compatibility with Stores pattern
export { Stores } from '@/core/stores'
