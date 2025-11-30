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

  /** Function to clear message text (set by TextInput) */
  clearMessage: (() => void) | null

  /** Register getter function (called by TextInput on mount) */
  setGetMessage: (getter: () => string) => void

  /** Register clear function (called by TextInput on mount) */
  setClearMessage: (clearer: () => void) => void

  /** Get current text value via stored getter */
  getText: () => string

  /** Clear text value via stored clearer */
  clearText: () => void
}

export const createTextStore = () =>
  createExtensionStore<TextStore>((set, get) => ({
    getMessage: null,
    clearMessage: null,

    setGetMessage: (getter: () => string) => {
      set(state => {
        state.getMessage = getter
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

    clearText: () => {
      const { clearMessage } = get()
      if (!clearMessage) {
        console.warn('[TextStore] clearMessage function not registered')
        return
      }
      clearMessage()
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
