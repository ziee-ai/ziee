import type { StoreSet } from '@ziee/framework/store-kit'

/**
 * TextStore state — manages composer text via getter/setter FUNCTIONS. Instead
 * of storing the Form instance directly (immer would freeze it), we store
 * functions that access the form. The form stays in the TextInput component;
 * these functions capture it via closure.
 */
export const textStoreState = {
  /** Function to get current message text (set by TextInput). */
  getMessage: null as (() => string) | null,
  /** Function to set message text (set by TextInput). */
  setMessage: null as ((text: string) => void) | null,
  /** Function to clear message text (set by TextInput). */
  clearMessage: null as (() => void) | null,
  /** Backup of message text (for error recovery). */
  backupMessage: null as string | null,
}

export type TextStoreState = typeof textStoreState
export type TextStoreSet = StoreSet<TextStoreState>
export type TextStoreGet = () => TextStoreState
