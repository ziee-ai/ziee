import type { Assistant } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const NEW_CHAT_ASSISTANT_KEY = '__new_chat__'

/**
 * The new-chat assistant key for a pane — a split pane gets its own
 * suffixed key so two new-chat panes don't share one assistant; a null paneId
 * (single-pane) keeps the bare `NEW_CHAT_ASSISTANT_KEY` (byte-identical).
 */
export const newChatAssistantKey = (
  paneId: string | null | undefined,
): string =>
  paneId ? `${NEW_CHAT_ASSISTANT_KEY}:${paneId}` : NEW_CHAT_ASSISTANT_KEY

export const assistantPickerState = {
  /** Selected assistant id per conversation key (value null = "no assistant"). */
  selectedByConversation: {} as Record<string, string | null>,
  /** Cached list of assistants the user can pick from (GLOBAL catalog). */
  availableAssistants: [] as Assistant[],
  loading: false,
  error: null as string | null,
}

export type AssistantPickerState = typeof assistantPickerState
export type AssistantPickerSet = StoreSet<AssistantPickerState>
export type AssistantPickerGet = () => AssistantPickerState
