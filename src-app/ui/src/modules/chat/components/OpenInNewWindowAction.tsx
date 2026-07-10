import { Button } from '@/components/ui'
import { SquareArrowOutUpRight } from 'lucide-react'
import { Stores } from '@/core/stores'
import { openConversationWindow } from '@/modules/chat/core/popout/openConversationWindow'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { useClosePane } from '@/modules/chat/core/pane/useOpenConversation'

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
 * live in two competing places. On the single-pane route it just opens the
 * window. The pane's own conversation is read via `useChatPane()` (not the
 * focused-pane bridge), so popping out pane B never pops out pane A.
 */
export function OpenInNewWindowAction() {
  const pane = useChatPaneOrNull()
  const closePane = useClosePane()
  // In a pane, act on THAT pane's conversation; single-pane reads the bridge.
  const conversation = pane
    ? pane.store.conversation
    : Stores.Chat.conversation
  if (!conversation) return null

  const isDesktop = typeof window !== 'undefined' && '__TAURI__' in window
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
