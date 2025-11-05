import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import type { Assistant, CreateAssistantRequest, UpdateAssistantRequest } from '@/api-client/types'

// Enable Map and Set support in Immer
enableMapSet()

interface UserAssistantsState {
  // Data
  assistants: Map<string, Assistant>
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

export const useUserAssistantsStore = create<UserAssistantsState>()(
  subscribeWithSelector(
    immer(
      (): UserAssistantsState => ({
        // Initial state
        assistants: new Map<string, Assistant>(),
        isInitialized: false,
        loading: false,
        creating: false,
        updating: false,
        deleting: false,
        error: null,
        __init__: {
          assistants: () => loadUserAssistants(),
        },
      }),
    ),
  ),
)

// User assistants actions
export const loadUserAssistants = async (): Promise<void> => {
  const state = useUserAssistantsStore.getState()
  if (state.isInitialized || state.loading) {
    return
  }
  try {
    useUserAssistantsStore.setState({ loading: true, error: null })

    const response = await ApiClient.Assistant.listUser({
      page: 1,
      limit: 50,
    })

    useUserAssistantsStore.setState({
      assistants: new Map(
        response.assistants.map((assistant: Assistant) => [assistant.id, assistant]),
      ),
      isInitialized: true,
      loading: false,
    })
  } catch (error) {
    useUserAssistantsStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to load assistants',
      loading: false,
    })
    throw error
  }
}

export const createUserAssistant = async (
  data: CreateAssistantRequest,
): Promise<Assistant> => {
  try {
    useUserAssistantsStore.setState({ creating: true, error: null })

    const assistant = await ApiClient.Assistant.createUser(data)

    useUserAssistantsStore.setState(state => {
      if (data.is_default) {
        // Set all other assistants' is_default to false
        state.assistants.forEach((a: Assistant) => {
          a.is_default = false
        })
      }
      // Add the new assistant
      state.assistants.set(assistant.id, assistant)
      state.creating = false
    })

    return assistant
  } catch (error) {
    useUserAssistantsStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to create assistant',
      creating: false,
    })
    throw error
  }
}

export const updateUserAssistant = async (
  id: string,
  data: UpdateAssistantRequest,
): Promise<Assistant> => {
  try {
    useUserAssistantsStore.setState({ updating: true, error: null })

    const assistant = await ApiClient.Assistant.updateUser({
      id,
      ...data,
    })

    useUserAssistantsStore.setState(state => {
      if (data.is_default) {
        // Set all other assistants' is_default to false
        state.assistants.forEach((a: Assistant, assistantId: string) => {
          if (assistantId !== id) {
            a.is_default = false
          }
        })
      }
      // Update the assistant
      state.assistants.set(id, assistant)
      state.updating = false
    })

    return assistant
  } catch (error) {
    useUserAssistantsStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to update assistant',
      updating: false,
    })
    throw error
  }
}

export const deleteUserAssistant = async (id: string): Promise<void> => {
  try {
    useUserAssistantsStore.setState({ deleting: true, error: null })

    await ApiClient.Assistant.deleteUser({ id })

    useUserAssistantsStore.setState(state => {
      state.assistants.delete(id)
      state.deleting = false
    })
  } catch (error) {
    useUserAssistantsStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to delete assistant',
      deleting: false,
    })
    throw error
  }
}

export const clearUserAssistantsStoreError = (): void => {
  useUserAssistantsStore.setState({ error: null })
}

// Helper to get default user assistant
export const getUserDefaultAssistant = (): Assistant | undefined => {
  return Array.from(useUserAssistantsStore.getState().assistants.values())
    .find(a => a.is_default)
}
