/**
 * Pure decision for "snap a closed pop-out window's conversation back into the main
 * window as a pane" (ITEM-54 / FB-12). When a native pop-out window closes, its
 * conversation should return to the main window — but never duplicated, and never
 * past the pane cap. Extracted pure so the snap-back logic is unit-testable without
 * the Tauri cross-window event runtime (which delivers the close signal).
 *
 * - `add`          → open it as a NEW pane (seeds a split from single-pane; the
 *                    reconcile reducer handles the actual placement via 'newPane').
 * - `alreadyShown` → it's already in a pane, or IS the single-pane conversation →
 *                    do not duplicate (the reconcile reducer would focus it anyway).
 * - `atCap`        → the main window already holds MAX_PANES → cannot add another;
 *                    skip (the workspace is full). Normally unreachable because
 *                    popping a pane OUT decrements the count, but guarded for the
 *                    case the user opened more panes while the window was open.
 */
export type SnapBackPlan = 'add' | 'alreadyShown' | 'atCap'

export function planPopoutSnapBack(
  conversationId: string,
  ctx: {
    /** conversationIds of the main window's current split panes (null = a new-chat pane). */
    paneConversationIds: Array<string | null>
    /** the single-pane (no-split) conversation the main window shows, if any. */
    singlePaneConversationId: string | null
    maxPanes: number
  },
): SnapBackPlan {
  if (ctx.paneConversationIds.includes(conversationId)) return 'alreadyShown'
  if (
    ctx.paneConversationIds.length === 0 &&
    ctx.singlePaneConversationId === conversationId
  ) {
    return 'alreadyShown'
  }
  if (ctx.paneConversationIds.length >= ctx.maxPanes) return 'atCap'
  return 'add'
}

/**
 * The main window's handler for "a pop-out window closed" (ITEM-54). Applies the
 * pure `planPopoutSnapBack` decision and, when the plan is `add`, opens the
 * conversation back into the main window as a new pane. Injected deps keep the
 * control flow (decision → action) unit-testable without the Tauri cross-window
 * event runtime (which merely DELIVERS the `conversationId`) or the
 * `useOpenConversationInWorkspace` React hook. Co-located with its decision so the
 * node:test module stays self-contained (no cross-local-import to resolve).
 */
export interface PopoutSnapBackDeps {
  /** conversationIds of the main window's current split panes (null = a new-chat pane). */
  getPaneConversationIds: () => Array<string | null>
  /** the single-pane (no-split) conversation the main window currently shows. */
  getSinglePaneConversationId: () => string | null
  maxPanes: number
  /** open the conversation back into the workspace as a NEW pane (reconcile 'newPane'). */
  openAsNewPane: (conversationId: string) => void
}

export function handlePopoutClosed(
  conversationId: string,
  deps: PopoutSnapBackDeps,
): void {
  const plan = planPopoutSnapBack(conversationId, {
    paneConversationIds: deps.getPaneConversationIds(),
    singlePaneConversationId: deps.getSinglePaneConversationId(),
    maxPanes: deps.maxPanes,
  })
  if (plan === 'add') deps.openAsNewPane(conversationId)
}
