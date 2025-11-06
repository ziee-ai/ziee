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
  modelError: Record<string, string> // providerId -> error message
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
      modelError: {},
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

    const providers = response.providers

    // Set providers immediately without models
    useLlmProviderStore.setState({
      providers: providers.map(p => ({ ...p, llm_models: [] })),
      isInitialized: true,
      loading: false,
    })

    // Fetch models for each provider in parallel
    const modelPromises = providers.map(async provider => {
      try {
        const modelsResponse = await ApiClient.LlmModel.list({
          providerId: provider.id,
          page: 1,
          perPage: 100,
        })
        return { providerId: provider.id, models: modelsResponse.models }
      } catch (error) {
        console.error(
          `Failed to load models for provider ${provider.id}:`,
          error,
        )
        return { providerId: provider.id, models: [] }
      }
    })

    const results = await Promise.allSettled(modelPromises)

    // Update each provider with its models
    const providersWithModels = providers.map(provider => {
      const result = results.find(
        r => r.status === 'fulfilled' && r.value.providerId === provider.id,
      )
      const models = result?.status === 'fulfilled' ? result.value.models : []
      return { ...provider, llm_models: models }
    })

    useLlmProviderStore.setState({
      providers: providersWithModels,
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

export const loadModelsForProvider = async (
  providerId: string,
): Promise<void> => {
  try {
    useLlmProviderStore.setState(state => ({
      llmModelsLoading: { ...state.llmModelsLoading, [providerId]: true },
      modelError: { ...state.modelError, [providerId]: '' },
    }))

    const modelsResponse = await ApiClient.LlmModel.list({
      providerId,
      page: 1,
      perPage: 100,
    })

    // Update provider with fresh models
    useLlmProviderStore.setState(state => ({
      providers: state.providers.map(p =>
        p.id === providerId ? { ...p, llm_models: modelsResponse.models } : p,
      ),
      llmModelsLoading: { ...state.llmModelsLoading, [providerId]: false },
    }))
  } catch (error) {
    const errorMessage =
      error instanceof Error ? error.message : 'Failed to load models'
    console.error(`Failed to load models for provider ${providerId}:`, error)
    useLlmProviderStore.setState(state => ({
      llmModelsLoading: { ...state.llmModelsLoading, [providerId]: false },
      modelError: { ...state.modelError, [providerId]: errorMessage },
    }))
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

    // Find existing provider to preserve llm_models
    const existingProvider = state.providers.find(p => p.id === id)
    const updatedProvider: LlmProviderWithModels = {
      ...provider,
      llm_models: existingProvider?.llm_models || [],
    }

    useLlmProviderStore.setState(state => ({
      providers: state.providers.map(p => (p.id === id ? updatedProvider : p)),
      updating: false,
    }))

    return updatedProvider
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

    useLlmProviderStore.setState(state => {
      // Clean up loading and error states for this provider
      const { [id]: _loading, ...remainingLoading } = state.llmModelsLoading
      const { [id]: _error, ...remainingErrors } = state.modelError

      return {
        providers: state.providers.filter(p => p.id !== id),
        llmModelsLoading: remainingLoading,
        modelError: remainingErrors,
        deleting: false,
      }
    })
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

export const findLlmProviderById = (
  id: string,
): LlmProviderWithModels | undefined => {
  return useLlmProviderStore.getState().providers.find(p => p.id === id)
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
      error: error instanceof Error ? error.message : 'Failed to enable model',
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
      error: error instanceof Error ? error.message : 'Failed to disable model',
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
      error: error instanceof Error ? error.message : 'Failed to delete model',
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

// Re-export drawer store hooks
export { useLlmProviderDrawerStore } from './drawer-store'

// Re-export llm-model drawer store hooks
export {
  useAddLocalLlmModelUploadDrawerStore,
  useAddLocalLlmModelDownloadDrawerStore,
  useEditLlmModelDrawerStore,
  useAddRemoteLlmModelDrawerStore,
  useViewDownloadDrawerStore,
} from './llm-model-drawer-store'

// Re-export download store hook
export { useLlmModelDownloadStore } from './llm-model-download-store'

// Re-export upload store hook
export { useUploadStore } from './llm-model-upload-store'

// Re-export for compatibility with Stores pattern
export { Stores } from '@/core/stores'
