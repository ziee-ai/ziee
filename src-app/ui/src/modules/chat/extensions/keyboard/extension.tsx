import { useEffect } from 'react'
import {
  createExtension,
  type ChatExtension,
} from '@/modules/chat/core/extensions'
import { useMainContentMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

/**
 * Keyboard shortcut configuration
 */
interface KeyboardShortcut {
  key: string
  ctrlKey?: boolean
  shiftKey?: boolean
  metaKey?: boolean
  altKey?: boolean
  description: string
  action: () => void
}

/**
 * Default keyboard shortcuts for chat
 */
const defaultShortcuts: KeyboardShortcut[] = [
  {
    key: 'Enter',
    ctrlKey: true,
    description: 'Send message (Ctrl+Enter)',
    action: () => {
      // Trigger send message
      const sendButton = document.querySelector<HTMLButtonElement>(
        '[data-testid="send-message-button"]',
      )
      if (sendButton) {
        sendButton.click()
      }
    },
  },
  {
    key: 'k',
    ctrlKey: true,
    description: 'Focus message input (Ctrl+K)',
    action: () => {
      const textarea = document.querySelector<HTMLTextAreaElement>(
        'textarea[placeholder*="Type your message"]',
      )
      if (textarea) {
        textarea.focus()
      }
    },
  },
  {
    key: 'Escape',
    description: 'Clear message input (Esc)',
    action: () => {
      const textarea = document.querySelector<HTMLTextAreaElement>(
        'textarea[placeholder*="Type your message"]',
      )
      if (textarea) {
        textarea.value = ''
        textarea.dispatchEvent(new Event('input', { bubbles: true }))
      }
    },
  },
]

/**
 * Global keyboard event handler
 */
let globalKeyboardHandler: ((event: KeyboardEvent) => void) | null = null

/**
 * Create keyboard event handler for shortcuts
 */
function createKeyboardHandler(
  shortcuts: KeyboardShortcut[],
): (event: KeyboardEvent) => void {
  return (event: KeyboardEvent) => {
    for (const shortcut of shortcuts) {
      const matches =
        event.key === shortcut.key &&
        !!event.ctrlKey === !!shortcut.ctrlKey &&
        !!event.shiftKey === !!shortcut.shiftKey &&
        !!event.metaKey === !!shortcut.metaKey &&
        !!event.altKey === !!shortcut.altKey

      if (matches) {
        event.preventDefault()
        event.stopPropagation()
        shortcut.action()
        break
      }
    }
  }
}

/**
 * Keyboard shortcuts help component
 */
function KeyboardShortcutsHelp() {
  const mainContentMinSize = useMainContentMinSize()
  if (mainContentMinSize.xs) return null

  return (
    <div className="text-xs text-muted-foreground">
      <span>Tips: Ctrl+Enter to send, Ctrl+K to focus, Esc to clear</span>
    </div>
  )
}

/**
 * Keyboard Extension
 * Provides keyboard shortcuts for chat interactions
 */
const keyboardExtension: ChatExtension = createExtension({
  name: 'keyboard',
  description: 'Provides keyboard shortcuts for chat',
  priority: 90,

  // No store needed - stateless extension

  initialize: async () => {
    // Only register handler if not already registered (global handler)
    if (!globalKeyboardHandler) {
      globalKeyboardHandler = createKeyboardHandler(defaultShortcuts)
      document.addEventListener('keydown', globalKeyboardHandler)

      console.log(
        '[Keyboard Extension] Initialized shortcuts:',
        defaultShortcuts.length,
      )
    }
  },

  cleanup: async () => {
    // Remove global handler
    if (globalKeyboardHandler) {
      document.removeEventListener('keydown', globalKeyboardHandler)
      globalKeyboardHandler = null
      console.log('[Keyboard Extension] Cleaned up shortcuts')
    }
  },

  // Register slot components
  slots: {
    toolbar_actions: { component: KeyboardShortcutsHelp, order: 90 },
  },
})

export default keyboardExtension

/**
 * Hook to use keyboard shortcuts in components
 */
export function useKeyboardShortcuts(shortcuts: KeyboardShortcut[]) {
  useEffect(() => {
    const handler = createKeyboardHandler(shortcuts)
    document.addEventListener('keydown', handler)

    return () => {
      document.removeEventListener('keydown', handler)
    }
  }, [shortcuts])
}
