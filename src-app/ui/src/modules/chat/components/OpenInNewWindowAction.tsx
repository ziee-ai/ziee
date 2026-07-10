import { Button } from '@/components/ui'
import { SquareArrowOutUpRight } from 'lucide-react'
import { Stores } from '@/core/stores'
import { openConversationWindow } from '@/modules/chat/core/popout/openConversationWindow'

/**
 * "Open in new window / tab" — registered into the `chatConversationHeaderTrailing`
 * slot (same mechanism the desktop `ConversationMountsControl` uses), so it renders
 * next to the TitleEditor in the conversation header on BOTH web and desktop.
 *
 * Opens the current conversation as a new top-level window/tab (a fresh SPA
 * instance with its own singleton stores) via `openConversationWindow` — web =
 * `window.open`, desktop = a native `WebviewWindow` (local-override). Copy adapts
 * per platform.
 *
 * (Pop-out ships BEFORE the in-window split, so it reads the single
 * `Stores.Chat.conversation`; the split tranche later re-scopes header actions to
 * the pane via `useChatPane()`.)
 */
export function OpenInNewWindowAction() {
  const { conversation } = Stores.Chat
  if (!conversation) return null

  const isDesktop = typeof window !== 'undefined' && '__TAURI__' in window
  const label = isDesktop ? 'Open in new window' : 'Open in new tab'

  return (
    <Button
      variant="ghost"
      size="default"
      className="!w-7 !h-7 !min-w-0 !p-0 opacity-70 hover:opacity-100"
      icon={<SquareArrowOutUpRight className="size-4" />}
      title={label}
      aria-label={label}
      data-testid="chat-open-in-new-window"
      onClick={() =>
        void openConversationWindow(conversation.id, {
          title: conversation.title ?? undefined,
        })
      }
    />
  )
}
