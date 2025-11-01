import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  LlmProvider as BaseLlmProvider,
  CreateLlmProviderRequest,
  UpdateLlmProviderRequest,
  LlmModel,
} from '@/api-client/types'

// Extended type that includes models array
// TODO: Backend should include llm_models in LlmProvider response
export interface LlmProviderWithModels extends BaseLlmProvider {
  llm_models?: LlmModel[]
}

interface LlmProviderState {
  // Data
  providers: LlmProviderWithModels[]
  isInitialized: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // LLM Model loading states
  llmModelsLoading: Record<string, boolean> // providerId -> loading
  llmModelOperations: Record<string, boolean> // modelId -> operation in progress

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
      llmModelsLoading: {},
      llmModelOperations: {},
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

    // Initialize each provider with llm_models array
    // TODO: Backend should include models in provider response or provide a way to fetch them
    const providersWithModels: LlmProviderWithModels[] = response.providers.map(p => ({
      ...p,
      llm_models: [],
    }))

    useLlmProviderStore.setState({
      providers: providersWithModels,
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
): Promise<LlmProviderWithModels> => {
  const state = useLlmProviderStore.getState()
  if (state.creating) {
    return Promise.resolve(null as any)
  }

  try {
    useLlmProviderStore.setState({ creating: true, error: null })

    const provider = await ApiClient.LlmProvider.create(data)

    // Add llm_models array to provider
    const providerWithModels: LlmProviderWithModels = {
      ...provider,
      llm_models: [],
    }

    useLlmProviderStore.setState(state => ({
      providers: [...state.providers, providerWithModels],
      creating: false,
    }))

    return providerWithModels
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
): Promise<LlmProviderWithModels> => {
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
      providers: state.providers.map(p =>
        p.id === id ? { ...provider, llm_models: p.llm_models } : p
      ),
      updating: false,
    }))

    return state.providers.find(p => p.id === id)!
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

export const findLlmProviderById = (id: string): LlmProviderWithModels | undefined => {
  return useLlmProviderStore
    .getState()
    .providers.find(p => p.id === id)
}

export const llmProviderHasCredentials = (
  provider: BaseLlmProvider | LlmProviderWithModels,
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

// LLM Model actions
export const enableLlmModel = async (modelId: string): Promise<LlmModel> => {
  try {
    useLlmProviderStore.setState(state => ({
      llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
      error: null,
    }))

    const model = await ApiClient.LlmModel.update({
      model_id: modelId,
      enabled: true,
    })

    // Update the model in the provider's llm_models array
    useLlmProviderStore.setState(state => ({
      providers: state.providers.map(p => ({
        ...p,
        llm_models: p.llm_models?.map(m => (m.id === modelId ? model : m)),
      })),
      llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
    }))

    return model
  } catch (error) {
    useLlmProviderStore.setState(state => ({
      error:
        error instanceof Error ? error.message : 'Failed to enable model',
      llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
    }))
    throw error
  }
}

export const disableLlmModel = async (modelId: string): Promise<LlmModel> => {
  try {
    useLlmProviderStore.setState(state => ({
      llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
      error: null,
    }))

    const model = await ApiClient.LlmModel.update({
      model_id: modelId,
      enabled: false,
    })

    // Update the model in the provider's llm_models array
    useLlmProviderStore.setState(state => ({
      providers: state.providers.map(p => ({
        ...p,
        llm_models: p.llm_models?.map(m => (m.id === modelId ? model : m)),
      })),
      llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
    }))

    return model
  } catch (error) {
    useLlmProviderStore.setState(state => ({
      error:
        error instanceof Error ? error.message : 'Failed to disable model',
      llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
    }))
    throw error
  }
}

export const deleteLlmModel = async (modelId: string): Promise<void> => {
  try {
    useLlmProviderStore.setState(state => ({
      llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
      error: null,
    }))

    await ApiClient.LlmModel.delete({ model_id: modelId })

    // Remove the model from the provider's llm_models array
    useLlmProviderStore.setState(state => ({
      providers: state.providers.map(p => ({
        ...p,
        llm_models: p.llm_models?.filter(m => m.id !== modelId),
      })),
      llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
    }))
  } catch (error) {
    useLlmProviderStore.setState(state => ({
      error:
        error instanceof Error ? error.message : 'Failed to delete model',
      llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
    }))
    throw error
  }
}

export const findLlmModelById = (modelId: string): LlmModel | undefined => {
  const state = useLlmProviderStore.getState()
  for (const provider of state.providers) {
    const model = provider.llm_models?.find(m => m.id === modelId)
    if (model) return model
  }
  return undefined
}

export const addLlmModelToProvider = (
  providerId: string,
  model: LlmModel,
): void => {
  useLlmProviderStore.setState(state => ({
    providers: state.providers.map(p => {
      if (p.id === providerId) {
        return {
          ...p,
          llm_models: [...(p.llm_models || []), model],
        }
      }
      return p
    }),
  }))
}

export const updateLlmModelInProvider = (
  providerId: string,
  modelId: string,
  updatedModel: LlmModel,
): void => {
  useLlmProviderStore.setState(state => ({
    providers: state.providers.map(p => {
      if (p.id === providerId) {
        return {
          ...p,
          llm_models: p.llm_models?.map(m =>
            m.id === modelId ? updatedModel : m,
          ),
        }
      }
      return p
    }),
  }))
}

// Re-export drawer store functions and hooks
export { useLlmProviderDrawerStore, openLlmProviderDrawer, closeLlmProviderDrawer } from './drawer-store'

// Re-export llm-model drawer stores
export * from './llm-model-drawer-store'

// Re-export download store functions
export {
  downloadLlmModelFromRepository,
  cancelLlmModelDownload,
  deleteLlmModelDownload,
  clearLlmModelDownload,
  clearAllLlmModelDownloads,
  useLlmModelDownloadStore,
} from './llm-model-download-store'

// Re-export for compatibility with Stores pattern
export { Stores } from '@/core/stores'
