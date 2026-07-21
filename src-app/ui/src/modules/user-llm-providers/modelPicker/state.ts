import type { StoreSet } from '@ziee/framework/store-kit'
import type { ProviderWithModels } from '@/api-client/types'

/**
 * ModelPicker store state.
 *
 * The composer key for a not-yet-created (new-chat) conversation. A pane with no
 * conversation yet selects its model under this key; on send the created
 * conversation adopts it (see the model chat-extension's composeRequestFields).
 * Two simultaneous new-chat panes share this one slot (accepted edge — the
 * per-pane split value is comparing EXISTING conversations' models).
 */
export const NEW_CHAT_MODEL_KEY = '__new_chat__'

/**
 * The new-chat model-selection key for a pane (ITEM-37). Two new-chat panes must
 * NOT share one model selection, so a split pane gets its own suffixed key; a
 * null paneId (single-pane) keeps the bare `NEW_CHAT_MODEL_KEY` (byte-identical).
 * A pane with no explicit pick still falls back to the shared default via
 * `defaultModelId()`.
 */
export const newChatModelKey = (paneId: string | null | undefined): string =>
  paneId ? `${NEW_CHAT_MODEL_KEY}:${paneId}` : NEW_CHAT_MODEL_KEY

export const modelPickerState = {
  /** User-accessible providers from the chat endpoint (GLOBAL catalog). */
  providers: [] as ProviderWithModels[],
  loading: false,
  error: null as string | null,
  /** Selected model ID (UUID) per conversation key (convId | NEW_CHAT_MODEL_KEY). */
  selectedByConversation: {} as Record<string, string>,
}

export type ModelPickerState = typeof modelPickerState
export type ModelPickerSet = StoreSet<ModelPickerState>
export type ModelPickerGet = () => ModelPickerState
