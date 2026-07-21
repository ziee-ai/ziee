import { defineExtensionStore } from '@/modules/chat/core/extensions'
import { textStoreState, type TextStoreState } from './state'
import type { Actions } from './actions.gen'

/**
 * TextStore — manages composer text via getter/setter functions, folder-glob
 * lazy-store pattern (`state.ts` + `actions/*.ts` + this index). Uses the EAGER
 * glob form (`{ eager: true }`): its `getText()` / `getBackupMessage()`
 * selectors return values consumed SYNCHRONOUSLY in handlers (e.g. the voice
 * engine reads `textStore.getText()` off the owning pane), so the actions must
 * load eagerly rather than behind a deferred dynamic import. Injected + read as
 * `Chat.TextStore`.
 */
export const createTextStore = defineExtensionStore<TextStoreState, Actions>({
  immer: true,
  state: textStoreState,
  actions: import.meta.glob('./actions/*.ts', { eager: true }),
})

/** Augment ChatExtensionStores with TextStore. */
declare module '../../../types' {
  interface ChatExtensionStores {
    TextStore: ReturnType<typeof createTextStore>
  }
}
