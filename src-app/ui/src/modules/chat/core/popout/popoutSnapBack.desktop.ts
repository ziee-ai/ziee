/**
 * Desktop override of the pop-out snap-back wiring (ITEM-54 / FB-12) — resolved by
 * `localOverridePlugin` in the desktop bundle.
 *
 * Two ends of one cross-window contract, over a single Tauri app-event:
 *  - `registerPopoutCloseEmitter` runs in a POP-OUT window (the `/chat-window/:id`
 *    route): on the window's close it emits `POPOUT_CLOSED_EVENT` carrying its
 *    conversationId.
 *  - `registerMainWindowSnapBackListener` runs ONCE in the MAIN window: it listens
 *    for that event and runs `handlePopoutClosed`, which snaps the conversation back
 *    into the workspace as a pane (never duplicated, never past the cap).
 *
 * The pure decision + handler (`planPopoutSnapBack` / `handlePopoutClosed`) are
 * unit-tested; the emit/listen control flow here is unit-tested with the Tauri
 * boundary mocked (TEST-83). The actual cross-OS-window event DELIVERY is a Tauri
 * platform guarantee (verified on the desktop host), not logic this file owns.
 */
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { emit, listen } from '@tauri-apps/api/event'
import {
  handlePopoutClosed,
  type PopoutSnapBackDeps,
} from './planPopoutSnapBack'

export type Unsubscribe = () => void

/** App-scoped event name (namespaced so it can't collide with other Tauri events). */
export const POPOUT_CLOSED_EVENT = 'ziee://popout-closed'

export async function registerPopoutCloseEmitter(
  conversationId: string,
): Promise<Unsubscribe> {
  const win = getCurrentWebviewWindow()
  // `onCloseRequested` fires while the window is closing — emit the snap-back signal
  // to the main window before this window goes away.
  const unlisten = await win.onCloseRequested(async () => {
    await emit(POPOUT_CLOSED_EVENT, { conversationId })
  })
  return unlisten
}

export async function registerMainWindowSnapBackListener(
  deps: PopoutSnapBackDeps,
): Promise<Unsubscribe> {
  return listen<{ conversationId: string }>(POPOUT_CLOSED_EVENT, event => {
    handlePopoutClosed(event.payload.conversationId, deps)
  })
}
