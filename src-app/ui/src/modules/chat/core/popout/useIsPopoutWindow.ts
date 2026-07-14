import { useLocation } from 'react-router-dom'

/**
 * True when the current view is the desktop pop-out WINDOW (the layout-less
 * `/chat-window/:conversationId` route, ITEM-52). The pop-out window is a focused
 * single-conversation view, so it hides window-management chrome — back / split /
 * pop-out (ITEM-55/56, FB-13) — that only makes sense in the main window. Single
 * source so every affordance keys off the same route check.
 */
export function useIsPopoutWindow(): boolean {
  return useLocation().pathname.startsWith('/chat-window/')
}
