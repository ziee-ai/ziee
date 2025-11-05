import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { Assistant, CreateAssistantRequest, UpdateAssistantRequest } from '@/api-client/types'

interface TemplateAssistantsState {
  // Data
  assistants: Assistant[]
  total: number
  currentPage: number
  pageSize: number
  isInitialized: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Error state
  error: string | null

  __init__: {
    assistants: () => Promise<void>
  }
}

export const useTemplateAssistantsStore = create<TemplateAssistantsState>()(
  subscribeWithSelector(
    immer(
      (): TemplateAssistantsState => ({
        // Initial state
        assistants: [],
        total: 0,
        currentPage: 1,
        pageSize: 10,
        isInitialized: false,
        loading: false,
        creating: false,
        updating: false,
        deleting: false,
        error: null,
        __init__: {
          assistants: () => loadTemplateAssistants(),
        },
      }),
    ),
  ),
)

// Template assistants actions
export const loadTemplateAssistants = async (
  page?: number,
  pageSize?: number,
): Promise<void> => {
  try {
    const currentState = useTemplateAssistantsStore.getState()
    const requestPage = page || currentState.currentPage
    const requestPageSize = pageSize || currentState.pageSize

    // Skip if already initialized and loading first page without explicit page parameter
    if (currentState.isInitialized && currentState.loading && !page) {
      return
    }

    useTemplateAssistantsStore.setState({ loading: true, error: null })

    const response = await ApiClient.AssistantTemplate.list({
      page: requestPage,
      limit: requestPageSize,
    })

    useTemplateAssistantsStore.setState({
      assistants: response.assistants,
      total: response.total,
      currentPage: requestPage,
      pageSize: requestPageSize,
      isInitialized: true,
      loading: false,
    })
  } catch (error) {
    useTemplateAssistantsStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to load template assistants',
      loading: false,
    })
    throw error
  }
}

export const createTemplateAssistant = async (
  data: CreateAssistantRequest,
): Promise<Assistant | undefined> => {
  const state = useTemplateAssistantsStore.getState()
  if (state.creating) {
    return
  }

  try {
    useTemplateAssistantsStore.setState({ creating: true, error: null })

    const assistant = await ApiClient.AssistantTemplate.create(data)

    useTemplateAssistantsStore.setState(state => ({
      assistants: data.is_default
        ? [
            ...state.assistants.map((a: Assistant) => ({ ...a, is_default: false })),
            assistant,
          ]
        : [...state.assistants, assistant],
      creating: false,
    }))

    return assistant
  } catch (error) {
    useTemplateAssistantsStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to create template assistant',
      creating: false,
    })
    throw error
  }
}

export const updateTemplateAssistant = async (
  id: string,
  data: UpdateAssistantRequest,
): Promise<Assistant | undefined> => {
  const state = useTemplateAssistantsStore.getState()
  if (state.updating) {
    return
  }

  try {
    useTemplateAssistantsStore.setState({ updating: true, error: null })

    const assistant = await ApiClient.AssistantTemplate.update({
      id,
      ...data,
    })

    useTemplateAssistantsStore.setState(state => ({
      assistants: data.is_default
        ? state.assistants.map((a: Assistant) =>
            a.id === id ? assistant : { ...a, is_default: false },
          )
        : state.assistants.map((a: Assistant) => (a.id === id ? assistant : a)),
      updating: false,
    }))

    return assistant
  } catch (error) {
    useTemplateAssistantsStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to update template assistant',
      updating: false,
    })
    throw error
  }
}

export const deleteTemplateAssistant = async (id: string): Promise<void> => {
  const state = useTemplateAssistantsStore.getState()
  if (state.deleting) {
    return
  }

  try {
    useTemplateAssistantsStore.setState({ deleting: true, error: null })

    await ApiClient.AssistantTemplate.delete({ id })

    useTemplateAssistantsStore.setState(state => ({
      assistants: state.assistants.filter((a: Assistant) => a.id !== id),
      deleting: false,
    }))
  } catch (error) {
    useTemplateAssistantsStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to delete template assistant',
      deleting: false,
    })
    throw error
  }
}

export const clearTemplateAssistantsStoreError = (): void => {
  useTemplateAssistantsStore.setState({ error: null })
}

// Helper to get default template assistant
export const getTemplateDefaultAssistant = (): Assistant | undefined => {
  return useTemplateAssistantsStore.getState().assistants.find(a => a.is_default)
}
