import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  LlmRepository,
  CreateLlmRepositoryRequest,
  UpdateLlmRepositoryRequest,
  TestRepositoryConnectionRequest,
} from '@/api-client/types'

interface LlmRepositoryState {
  // Data
  repositories: LlmRepository[]
  isInitialized: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean
  testing: boolean

  // Error state
  error: string | null

  __init__: {
    repositories: () => Promise<void>
  }
}

export const useLlmRepositoryStore = create<LlmRepositoryState>()(
  subscribeWithSelector(
    (): LlmRepositoryState => ({
      // Initial state
      repositories: [],
      isInitialized: false,
      loading: false,
      creating: false,
      updating: false,
      deleting: false,
      testing: false,
      error: null,
      __init__: {
        repositories: async () => loadLlmRepositories(),
      },
    }),
  ),
)

// Repository actions
export const loadLlmRepositories = async (): Promise<void> => {
  const state = useLlmRepositoryStore.getState()
  if (state.isInitialized || state.loading) {
    return
  }
  try {
    useLlmRepositoryStore.setState({ loading: true, error: null })

    const response = await ApiClient.LlmRepository.list({
      page: 1,
      per_page: 50,
    })

    useLlmRepositoryStore.setState({
      repositories: response.repositories,
      isInitialized: true,
      loading: false,
    })
  } catch (error) {
    useLlmRepositoryStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to load repositories',
      loading: false,
    })
    throw error
  }
}

export const createLlmRepository = async (
  data: CreateLlmRepositoryRequest,
): Promise<LlmRepository> => {
  const state = useLlmRepositoryStore.getState()
  if (state.creating) {
    return Promise.resolve(null as any)
  }

  try {
    useLlmRepositoryStore.setState({ creating: true, error: null })

    const repository = await ApiClient.LlmRepository.create(data)

    useLlmRepositoryStore.setState(state => ({
      repositories: [...state.repositories, repository],
      creating: false,
    }))

    return repository
  } catch (error) {
    useLlmRepositoryStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to create repository',
      creating: false,
    })
    throw error
  }
}

export const updateLlmRepository = async (
  id: string,
  data: UpdateLlmRepositoryRequest,
): Promise<LlmRepository> => {
  const state = useLlmRepositoryStore.getState()
  if (state.updating) {
    return Promise.resolve(null as any)
  }

  try {
    useLlmRepositoryStore.setState({ updating: true, error: null })

    const repository = await ApiClient.LlmRepository.update({
      repository_id: id,
      ...data,
    })

    useLlmRepositoryStore.setState(state => ({
      repositories: state.repositories.map(r => (r.id === id ? repository : r)),
      updating: false,
    }))

    return repository
  } catch (error) {
    useLlmRepositoryStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to update repository',
      updating: false,
    })
    throw error
  }
}

export const deleteLlmRepository = async (id: string): Promise<void> => {
  const state = useLlmRepositoryStore.getState()
  if (state.deleting) {
    return
  }

  try {
    useLlmRepositoryStore.setState({ deleting: true, error: null })

    await ApiClient.LlmRepository.delete({ repository_id: id })

    useLlmRepositoryStore.setState(state => ({
      repositories: state.repositories.filter(r => r.id !== id),
      deleting: false,
    }))
  } catch (error) {
    useLlmRepositoryStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to delete repository',
      deleting: false,
    })
    throw error
  }
}

export const testLlmRepositoryConnection = async (
  data: TestRepositoryConnectionRequest,
): Promise<{ success: boolean; message: string }> => {
  const state = useLlmRepositoryStore.getState()
  if (state.testing) {
    return {
      success: false,
      message: 'Repository connection test already in progress',
    }
  }

  try {
    useLlmRepositoryStore.setState({ testing: true, error: null })

    const result = await ApiClient.LlmRepository.test(data)

    useLlmRepositoryStore.setState({ testing: false })

    return result
  } catch (error) {
    useLlmRepositoryStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to test repository connection',
      testing: false,
    })
    throw error
  }
}

export const clearLlmRepositoryStoreError = (): void => {
  useLlmRepositoryStore.setState({ error: null })
}

export const findLlmRepositoryById = (
  id: string,
): LlmRepository | undefined => {
  return useLlmRepositoryStore.getState().repositories.find(r => r.id === id)
}

export const llmRepositoryHasCredentials = (
  repository: LlmRepository,
): boolean => {
  // If auth type is none, no credentials are needed
  if (repository.auth_type === 'none') {
    return true
  }

  // Check if auth_config exists
  if (!repository.auth_config) {
    return false
  }

  // Check credentials based on auth type
  switch (repository.auth_type) {
    case 'api_key':
      return !!(
        repository.auth_config.api_key && repository.auth_config.api_key.trim()
      )

    case 'basic_auth':
      return !!(
        repository.auth_config.username &&
        repository.auth_config.username.trim() &&
        repository.auth_config.password &&
        repository.auth_config.password.trim()
      )

    case 'bearer_token':
      return !!(
        repository.auth_config.token && repository.auth_config.token.trim()
      )

    default:
      return false
  }
}
