import type { PopoutSnapBackDeps } from './planPopoutSnapBack'

/**
 * Web base of the pop-out snap-back wiring (ITEM-54 / FB-12). The web pop-out is a
 * browser tab, not a native window this app controls, so there is no cross-window
 * close signal to react to — both registrations are no-ops that return an inert
 * unsubscribe. The desktop override (`popoutSnapBack.desktop.ts`) wires the real
 * Tauri close event.
 */
export type Unsubscribe = () => void

/** POP-OUT window: emit a close signal when it closes. Web: no-op. */
export async function registerPopoutCloseEmitter(
  _conversationId: string,
): Promise<Unsubscribe> {
  return () => {}
}

/** MAIN window: snap a closed pop-out's conversation back as a pane. Web: no-op. */
export async function registerMainWindowSnapBackListener(
  _deps: PopoutSnapBackDeps,
): Promise<Unsubscribe> {
  return () => {}
}
