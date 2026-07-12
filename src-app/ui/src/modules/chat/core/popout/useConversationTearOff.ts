import { useCallback } from 'react'
import type { DragEvent } from 'react'
import { openConversationWindow } from '@/modules/chat/core/popout/openConversationWindow'
import { useClosePane } from '@/modules/chat/core/pane/useOpenConversation'
import {
  isOutsideWindow,
  planTearOff,
  runTearOffPlan,
} from '@/modules/chat/core/popout/tearOff'

/**
 * Tear-off wiring (ITEM-58) — attach the returned handler to a conversation drag
 * source's `onDragEnd` (sidebar item, conversation card, or a split pane's grip).
 * When the drag is released PAST the window edge on the DESKTOP, it opens the
 * conversation as its own native window (`openConversationWindow`, seam-resolved)
 * and, for a pane source, closes that pane (the MOVE semantics — ITEM-29). On web
 * or an in-window release it does nothing (the pure `planTearOff` gates it).
 *
 * Reads the release point from the `dragend` screen coords and the window rect
 * from `window.screen*` / `outerWidth|Height` — both are populated in a Tauri
 * webview exactly as in a browser, so the same code drives both; the desktop gate
 * (`__TAURI__`) is what keeps web a no-op.
 */
export function useConversationTearOff() {
  const closePane = useClosePane()
  return useCallback(
    (
      e: DragEvent,
      source: { conversationId: string; paneId?: string | null; title?: string },
    ) => {
      if (typeof window === 'undefined') return
      const isDesktop = '__TAURI__' in window
      const isOutside = isOutsideWindow(
        { screenX: e.screenX, screenY: e.screenY },
        {
          screenX: window.screenX,
          screenY: window.screenY,
          outerWidth: window.outerWidth,
          outerHeight: window.outerHeight,
        },
      )
      const plan = planTearOff({
        isOutside,
        isDesktop,
        conversationId: source.conversationId,
        paneId: source.paneId ?? null,
      })
      runTearOffPlan(plan, {
        openWindow: (id, opts) => void openConversationWindow(id, opts),
        closePane,
        title: source.title,
      })
    },
    [closePane],
  )
}
