import { createExtensionStore } from '../../core/extensions'

/**
 * TextStore
 * Manages text input via getter/setter functions
 *
 * Instead of storing the Form instance directly (which gets frozen by immer),
 * we store functions that access the form. The form stays in the TextInput
 * component, and these functions capture it via closure.
 */
interface TextStore {
  /** Function to get current message text (set by TextInput) */
  getMessage: (() => string) | null

  /** Function to set message text (set by TextInput) */
  setMessage: ((text: string) => void) | null

  /** Function to clear message text (set by TextInput) */
  clearMessage: (() => void) | null

  /** Backup of message text (for error recovery) */
  backupMessage: string | null

  /** Register getter function (called by TextInput on mount) */
  setGetMessage: (getter: () => string) => void

  /** Register setter function (called by TextInput on mount) */
  setSetMessage: (setter: (text: string) => void) => void

  /** Register clear function (called by TextInput on mount) */
  setClearMessage: (clearer: () => void) => void

  /** Get current text value via stored getter */
  getText: () => string

  /** Set text value via stored setter */
  setText: (text: string) => void

  /** Clear text value via stored clearer */
  clearText: () => void

  /** Set backup message (before clearing) */
  setBackupMessage: (text: string | null) => void

  /** Get backup message */
  getBackupMessage: () => string | null

  /** Restore text from backup */
  restoreFromBackup: () => void
}

export const createTextStore = () =>
  createExtensionStore<TextStore>((set, get) => ({
    getMessage: null,
    setMessage: null,
    clearMessage: null,
    backupMessage: null,

    setGetMessage: (getter: () => string) => {
      set(state => {
        state.getMessage = getter
      })
    },

    setSetMessage: (setter: (text: string) => void) => {
      set(state => {
        state.setMessage = setter
      })
    },

    setClearMessage: (clearer: () => void) => {
      set(state => {
        state.clearMessage = clearer
      })
    },

    getText: () => {
      const { getMessage } = get()
      if (!getMessage) {
        console.warn('[TextStore] getMessage function not registered')
        return ''
      }
      return getMessage()
    },

    setText: (text: string) => {
      const { setMessage } = get()
      if (!setMessage) {
        console.warn('[TextStore] setMessage function not registered')
        return
      }
      setMessage(text)
    },

    clearText: () => {
      const { clearMessage } = get()
      if (!clearMessage) {
        console.warn('[TextStore] clearMessage function not registered')
        return
      }
      clearMessage()
    },

    setBackupMessage: (text: string | null) => {
      set(state => {
        state.backupMessage = text
      })
    },

    getBackupMessage: () => {
      return get().backupMessage
    },

    restoreFromBackup: () => {
      const { backupMessage, setMessage } = get()
      if (backupMessage && setMessage) {
        setMessage(backupMessage)
        console.log('[TextStore] Restored text from backup')
      }
    },
  }))

/**
 * Augment ChatExtensionStores with TextStore
 */
declare module '../../types' {
  interface ChatExtensionStores {
    TextStore: ReturnType<typeof createTextStore>
  }
}
