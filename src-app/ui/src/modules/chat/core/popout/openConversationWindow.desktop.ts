/**
 * Desktop override for `openConversationWindow` — DELIBERATE DIVERGENCE from the
 * web base (`./openConversationWindow.ts`, which uses `window.open`).
 *
 * On the Tauri desktop we open a real OS window via `WebviewWindow` — native
 * taskbar/dock entry, traffic-light controls, resizable, persists across app
 * focus. The same Vite dev server + bundled SPA serves the `/chat/:id` route, so
 * the new window renders the existing single-conversation `ConversationPage` and
 * self-authenticates via the desktop-base `auto_login` boot (its own singleton
 * stores). Opening a native OS window is shell-native, so the desktop copy uses
 * the Tauri window API directly (NOT an Axum route).
 *
 * This is a whole-file co-located override: the desktop build's
 * `localOverridePlugin` resolves any `@/modules/chat/core/popout/
 * openConversationWindow` import to THIS `.desktop.ts` in the desktop bundle,
 * while the web bundle keeps the base file. (Migrated from the former raw shadow
 * at `src-app/desktop/ui/src/modules/chat/core/popout/openConversationWindow.ts`
 * to the live2 co-located `.desktop` mechanism — same idiom as
 * `api-client/getBaseURL.desktop.ts`.)
 *
 * Singleton per conversation: the label `chat-<id>` is reused on every open; if a
 * window with that label already exists, focus it instead of duplicating (the
 * same dedup contract as the web window-name).
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
      // Unminimize BEFORE focusing: on several window managers `setFocus()` on a
      // minimized window is a no-op / does not raise it, so re-triggering pop-out
      // on a minimized conversation window would leave it unraised. Restore first,
      // then focus.
      await existing.unminimize()
      await existing.setFocus()
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
