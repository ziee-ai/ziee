/**
 * Desktop override of `focusPopoutWindowIfOpen` (ITEM-53 / FB-12) — resolved by
 * `localOverridePlugin` in the desktop bundle.
 *
 * When the user opens a conversation from the MAIN window that is ALREADY live in a
 * native pop-out window, we must focus that existing window instead of opening the
 * conversation a second time inline. This looks up the pop-out window by its stable
 * `chat-<id>` label (the same label `openConversationWindow.desktop.ts` creates),
 * and if one exists, unminimizes + focuses it and reports that it handled the open.
 *
 * @returns `true` if an existing pop-out window was found + focused (the caller must
 *   NOT open the conversation inline); `false` if none exists (open inline as usual).
 */
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { popoutWindowLabel } from './popoutWindowLabel'

export async function focusPopoutWindowIfOpen(
  conversationId: string,
): Promise<boolean> {
  try {
    const existing = await WebviewWindow.getByLabel(popoutWindowLabel(conversationId))
    if (existing) {
      // Unminimize BEFORE focus (some WMs ignore setFocus on a minimized window),
      // mirroring the dedup path in openConversationWindow.desktop.ts.
      await existing.unminimize()
      await existing.setFocus()
      return true
    }
  } catch (error) {
    // A lookup/focus failure must not swallow the open — fall through to inline.
    console.error('[popout] focusPopoutWindowIfOpen failed:', error)
  }
  return false
}
