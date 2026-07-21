import { Button } from '@ziee/kit'
import { SquareArrowOutUpRight } from 'lucide-react'
import { openConversationWindow } from '@/modules/chat/core/popout/openConversationWindow'
import { popoutActionVisible } from '@/modules/chat/core/popout/popoutVisibility'
import { useIsPopoutWindow } from '@/modules/chat/core/popout/useIsPopoutWindow'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { useClosePane } from '@/modules/chat/core/pane/useOpenConversation'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * "Open in new window / tab" — registered into the `chatConversationHeaderTrailing`
 * slot (same mechanism the desktop `ConversationMountsControl` uses), so it renders
 * next to the TitleEditor in the conversation header on BOTH web and desktop.
 *
 * Opens the current conversation as a new top-level window/tab (a fresh SPA
 * instance with its own singleton stores) via `openConversationWindow` — web =
 * `window.open`, desktop = a native `WebviewWindow`.
 *
 * Inside a SPLIT pane this is a **pop-out that MOVES** (ITEM-29): it opens the
 * window AND removes the pane from the workspace, so the conversation is never
 * live in two competing places. In SINGLE-pane it opens the window but renders
 * ONLY on the desktop app (ITEM-44/DEC-60): a native OS window is the only way to
 * get one there, whereas on the web the browser's own "open in new tab" already
 * covers it, so the single-pane web button is hidden as redundant. The pane's own
 * conversation is read via `useChatPane()` (not the focused-pane bridge), so
 * popping out pane B never pops out pane A.
 */
export function OpenInNewWindowAction() {
  const pane = useChatPaneOrNull()
  const closePane = useClosePane()
  const isPopoutWindow = useIsPopoutWindow()
  // In a pane, act on THAT pane's conversation; single-pane reads the bridge.
  const conversation = pane
    ? pane.store.conversation
    : Chat.conversation
  if (!conversation) return null

  const isDesktop = typeof window !== 'undefined' && '__TAURI__' in window
  // Single-pane pop-out is desktop-only (ITEM-44/DEC-60); split panes always show it;
  // NEVER inside the pop-out window itself (ITEM-56/FB-13 — a self-focusing no-op).
  if (!popoutActionVisible(pane != null, isDesktop, isPopoutWindow)) return null

  const label = pane
    ? isDesktop
      ? 'Pop out to new window'
      : 'Pop out to new tab'
    : isDesktop
      ? 'Open in new window'
      : 'Open in new tab'

  return (
    <Button
      variant="ghost"
      size="default"
      className="!w-7 !h-7 !min-w-0 !p-0 opacity-70 hover:opacity-100"
      icon={<SquareArrowOutUpRight className="size-4" />}
      title={label}
      aria-label={label}
      data-testid="chat-open-in-new-window"
      onClick={() => {
        void openConversationWindow(conversation.id, {
          title: conversation.title ?? undefined,
        })
        // Pop-out MOVES the pane out of the workspace (no two live copies).
        if (pane) closePane(pane.paneId)
      }}
    />
  )
}
