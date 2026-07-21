import type { Assistant } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const assistantDrawerState = {
  open: false,
  loading: false,
  editingAssistant: null as Assistant | null,
  isTemplate: false,
  isCloning: false,
}

export type AssistantDrawerState = typeof assistantDrawerState
export type AssistantDrawerSet = StoreSet<AssistantDrawerState>
export type AssistantDrawerGet = () => AssistantDrawerState
