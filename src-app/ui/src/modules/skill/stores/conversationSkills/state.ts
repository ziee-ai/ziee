import type { AvailableSkillEntry } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const conversationSkillsState = {
  // Keyed by conversation id.
  available: {} as Record<string, AvailableSkillEntry[]>,
  loading: {} as Record<string, boolean>,
  error: null as string | null,
}

export type ConversationSkillsState = typeof conversationSkillsState
export type ConversationSkillsSet = StoreSet<ConversationSkillsState>
export type ConversationSkillsGet = () => ConversationSkillsState
