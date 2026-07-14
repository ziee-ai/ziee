import { useEffect } from 'react'
import {
  createExtension,
  type ChatExtension,
} from '@/modules/chat/core/extensions'
import { useMainContentMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { useSplitViewStore } from '@/modules/chat/core/stores/SplitView.store'

/**
 * Resolve the DOM subtree of the FOCUSED split pane (ITEM-39). The keyboard
 * shortcuts run one GLOBAL document listener, so a `document.querySelector` would
 * always hit the LEFTMOST pane's Send button / textarea — Ctrl+Enter would send
 * pane A no matter which pane you're typing in. Scoping the lookup to the focused
 * pane's container (`chat-pane-<index>`) sends/focuses/clears the RIGHT pane.
 * Single-pane (0–1 panes) has no wrapper → falls back to `document`, unchanged.
 */
function focusedPaneRoot(): ParentNode {
  const { panes, focusedPaneId } = useSplitViewStore.getState()
  if (panes.length < 2 || !focusedPaneId) return document
  const idx = panes.findIndex((p) => p.paneId === focusedPaneId)
  if (idx < 0) return document
  return (
    document.querySelector<HTMLElement>(`[data-testid="chat-pane-${idx}"]`) ??
    document
  )
}

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
      // Trigger send in the FOCUSED pane (ITEM-39), not the leftmost.
      const sendButton = focusedPaneRoot().querySelector<HTMLButtonElement>(
        'button[aria-label="Send message"]',
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
      const textarea = focusedPaneRoot().querySelector<HTMLTextAreaElement>(
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
      const textarea = focusedPaneRoot().querySelector<HTMLTextAreaElement>(
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
// Refcount of live pane runtimes (ITEM-39): the ONE global listener must survive
// until EVERY pane has cleaned up. Without it, the first pane's cleanup() would
// remove the shared listener and disarm the survivors.
let keyboardInitCount = 0

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
    <div
      className="text-xs text-muted-foreground truncate min-w-0"
      data-testid="chat-keyboard-tips"
    >
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
    // ONE global listener across all panes (refcounted). Actions resolve the
    // focused pane at event time (focusedPaneRoot), so a single handler is
    // pane-correct.
    keyboardInitCount++
    if (!globalKeyboardHandler) {
      globalKeyboardHandler = createKeyboardHandler(defaultShortcuts)
      document.addEventListener('keydown', globalKeyboardHandler)
    }
  },

  cleanup: async () => {
    // Only remove the shared listener once the LAST pane has cleaned up.
    keyboardInitCount = Math.max(0, keyboardInitCount - 1)
    if (keyboardInitCount === 0 && globalKeyboardHandler) {
      document.removeEventListener('keydown', globalKeyboardHandler)
      globalKeyboardHandler = null
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
