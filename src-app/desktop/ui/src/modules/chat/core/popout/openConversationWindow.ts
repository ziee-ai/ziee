/**
 * DELIBERATE DIVERGENCE from core's `openConversationWindow` (web `window.open`).
 *
 * On the Tauri desktop we open a real OS window via `WebviewWindow` — native
 * taskbar/dock entry, traffic-light controls, resizable, persists across app
 * focus. The same Vite dev server + bundled SPA serves the `/chat/:id` route, so
 * the new window renders the existing single-conversation `ConversationPage` and
 * self-authenticates via the desktop-base `auto_login` boot (its own singleton
 * stores). Resolved by `localOverridePlugin`: any `@/modules/chat/core/popout/
 * openConversationWindow` import lands here in the desktop bundle.
 *
 * Singleton per conversation: the label `chat-<id>` is reused on every open; if a
 * window with that label already exists, focus it instead of duplicating (the
 * same dedup contract as the web window-name, and mirrors HardwareMonitorButton).
 */
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

const labelFor = (conversationId: string) => `chat-${conversationId}`

export async function openConversationWindow(
  conversationId: string,
  opts?: { title?: string },
): Promise<void> {
  const label = labelFor(conversationId)
  try {
    const existing = await WebviewWindow.getByLabel(label)
    if (existing) {
      await existing.setFocus()
      await existing.unminimize()
      return
    }

    const win = new WebviewWindow(label, {
      url: `/chat/${conversationId}`,
      title: opts?.title ?? 'Conversation',
      width: 900,
      height: 720,
      minWidth: 480,
      minHeight: 400,
      resizable: true,
      center: true,
    })

    win.once('tauri://error', (e: unknown) => {
      console.error('[popout] conversation window failed to open:', e)
    })
  } catch (error) {
    console.error('[popout] openConversationWindow failed:', error)
  }
}
