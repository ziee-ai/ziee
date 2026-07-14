/**
 * The stable native-window label for a conversation's desktop pop-out window
 * (ITEM-53). Shared single source so BOTH `openConversationWindow.desktop.ts` (which
 * creates + dedups the window) and `focusPopoutWindow.desktop.ts` (which focuses an
 * already-open one from the MAIN window) key off the exact same label — a mismatch
 * would silently break dedup/focus. Pure, so it's unit-testable without Tauri.
 */
export const popoutWindowLabel = (conversationId: string): string =>
  `chat-${conversationId}`
