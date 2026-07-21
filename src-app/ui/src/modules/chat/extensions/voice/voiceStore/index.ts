import { defineExtensionStore } from '@/modules/chat/core/extensions'
import { voiceStoreState, type VoiceStoreState } from './state'
import type { Actions } from './actions.gen'

export type { VoiceStatus } from './state'
// Re-exported for MicButton (hides itself where recording is impossible).
export { isRecordingSupported } from './actions/_engine'

/**
 * VoiceStore — the chat-composer voice-dictation state machine, folder-glob
 * lazy-store pattern (`state.ts` + `actions/*.ts` + this index). The 5 actions
 * share delicate per-instance closures + module-scope imperative resources, so
 * they live together in `actions/_engine.ts` (one per instance, memoized) and
 * each `actions/<name>.ts` thin-delegates. `immer: false` — non-serializable
 * resources (MediaRecorder / MediaStream / timers) live in module scope.
 *
 * Read as `Chat.VoiceStore`; actions are callable directly, and
 * handler-side state reads use the `$` snapshot.
 */
export const createVoiceStore = defineExtensionStore<VoiceStoreState, Actions>({
  immer: false,
  state: voiceStoreState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    void actions.fetchCapability()
  },
})

/** Augment ChatExtensionStores with VoiceStore. */
declare module '../../../types' {
  interface ChatExtensionStores {
    VoiceStore: ReturnType<typeof createVoiceStore>
  }
}
