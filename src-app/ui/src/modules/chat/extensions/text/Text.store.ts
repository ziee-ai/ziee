import { defineExtensionStore } from '@/modules/chat/core/extensions'

/**
 * TextStore — manages composer text via getter/setter functions.
 *
 * Instead of storing the Form instance directly (immer would freeze it), we
 * store functions that access the form. The form stays in the TextInput
 * component; these functions capture it via closure. Migrated to
 * `defineExtensionStore` (store-kit authoring model) from the raw
 * `createExtensionStore((set,get)=>({...}))` factory — reads/writes (incl. `$`)
 * are unchanged; it's still injected + read as `Stores.Chat.TextStore`.
 */
export const createTextStore = defineExtensionStore({
  immer: true,
  state: {
    /** Function to get current message text (set by TextInput). */
    getMessage: null as (() => string) | null,
    /** Function to set message text (set by TextInput). */
    setMessage: null as ((text: string) => void) | null,
    /** Function to clear message text (set by TextInput). */
    clearMessage: null as (() => void) | null,
    /** Backup of message text (for error recovery). */
    backupMessage: null as string | null,
  },
  actions: (set, get) => ({
    /** Register getter function (called by TextInput on mount). */
    setGetMessage: (getter: () => string) => {
      set(state => {
        state.getMessage = getter
      })
    },
    /** Register setter function (called by TextInput on mount). */
    setSetMessage: (setter: (text: string) => void) => {
      set(state => {
        state.setMessage = setter
      })
    },
    /** Register clear function (called by TextInput on mount). */
    setClearMessage: (clearer: () => void) => {
      set(state => {
        state.clearMessage = clearer
      })
    },
    /** Get current text value via stored getter. */
    getText: (): string => {
      const { getMessage } = get()
      if (!getMessage) {
        console.warn('[TextStore] getMessage function not registered')
        return ''
      }
      return getMessage()
    },
    /** Set text value via stored setter. */
    setText: (text: string) => {
      const { setMessage } = get()
      if (!setMessage) {
        console.warn('[TextStore] setMessage function not registered')
        return
      }
      setMessage(text)
    },
    /** Clear text value via stored clearer. */
    clearText: () => {
      const { clearMessage } = get()
      if (!clearMessage) {
        console.warn('[TextStore] clearMessage function not registered')
        return
      }
      clearMessage()
    },
    /** Set backup message (before clearing). */
    setBackupMessage: (text: string | null) => {
      set(state => {
        state.backupMessage = text
      })
    },
    /** Get backup message. */
    getBackupMessage: (): string | null => get().backupMessage,
    /** Restore text from backup. */
    restoreFromBackup: () => {
      const { backupMessage, setMessage } = get()
      if (backupMessage && setMessage) {
        setMessage(backupMessage)
        console.log('[TextStore] Restored text from backup')
      }
    },
  }),
})

/**
 * Augment ChatExtensionStores with TextStore
 */
declare module '../../types' {
  interface ChatExtensionStores {
    TextStore: ReturnType<typeof createTextStore>
  }
}
