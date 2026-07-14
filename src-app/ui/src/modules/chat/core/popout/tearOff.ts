/**
 * Tear-off (ITEM-58) — dragging a conversation (from the sidebar or a split
 * pane's grip) and releasing it PAST the window edge opens it as its own window.
 * DESKTOP ONLY (DEC-70): the native OS window (`openConversationWindow`) is the
 * only place a torn-off window makes sense; on web the drag-release is ignored
 * and the existing ⤢ button remains. STRICT trigger (DEC-71): only a release
 * genuinely outside the window bounds tears off, so an in-window mis-drop never
 * spawns a window.
 *
 * Pure so the geometry + decision are unit-testable; `useConversationTearOff`
 * supplies the live `dragend` screen coords + the window rect and executes the
 * plan (open the window, and for a pane source MOVE it — close the pane).
 */

/** Screen-space release point of a `dragend`. */
export interface ReleasePoint {
  screenX: number
  screenY: number
}

/** The window's position + size in global screen coordinates. */
export interface WindowRect {
  screenX: number
  screenY: number
  outerWidth: number
  outerHeight: number
}

/**
 * True when the release point lies outside the window rect on any side. The
 * right/bottom edges are exclusive (`>=` is outside) so a release exactly on the
 * far edge counts as leaving; the left/top edges are inclusive of the origin.
 *
 * A DEGENERATE / unreliable rect (a non-positive `outerWidth`/`outerHeight`, or
 * any non-finite value) returns `false` — some webviews report
 * `outerWidth/Height = 0`, which would otherwise make an EMPTY inside-rect so
 * EVERY point reads as "outside" and every in-window release would spuriously
 * tear off. When we can't trust the geometry we do NOT tear off (blind-audit
 * robustness fix; the residual bogus-`(0,0)`-coord case is desktop-host verified).
 */
export function isOutsideWindow(release: ReleasePoint, win: WindowRect): boolean {
  if (
    !Number.isFinite(win.screenX) ||
    !Number.isFinite(win.screenY) ||
    !Number.isFinite(release.screenX) ||
    !Number.isFinite(release.screenY) ||
    !(win.outerWidth > 0) ||
    !(win.outerHeight > 0)
  ) {
    return false
  }
  return (
    release.screenX < win.screenX ||
    release.screenY < win.screenY ||
    release.screenX >= win.screenX + win.outerWidth ||
    release.screenY >= win.screenY + win.outerHeight
  )
}

export interface TearOffInput {
  isOutside: boolean
  isDesktop: boolean
  conversationId: string
  /** Set when the source is a split pane — the pane MOVES (closes) on tear-off. */
  paneId?: string | null
}

export interface TearOffPlan {
  /** Open a native window for this conversation. */
  open: boolean
  conversationId: string
  /** Non-null → close this pane after opening (the MOVE semantics, ITEM-29). */
  closePaneId: string | null
}

/**
 * Decide the tear-off action. Opens ONLY when the release was outside the window
 * AND we're on desktop (web releases are ignored — the ⤢ button covers web). A
 * pane source additionally closes its pane so the conversation is never live in
 * two competing places (mirrors the pop-out button's MOVE, ITEM-29).
 */
export function planTearOff(input: TearOffInput): TearOffPlan {
  const active = input.isOutside && input.isDesktop
  return {
    open: active,
    conversationId: input.conversationId,
    closePaneId: active && input.paneId ? input.paneId : null,
  }
}

/**
 * Execute a {@link TearOffPlan}: open the conversation window when the plan is
 * active, and (for a pane source) close the pane. The two effects are injected so
 * this glue is testable without a Tauri runtime or React render — the hook passes
 * the real `openConversationWindow` seam + `closePane`. Returns whether it acted.
 */
export function runTearOffPlan(
  plan: TearOffPlan,
  effects: {
    openWindow: (id: string, opts?: { title?: string }) => void
    closePane: (paneId: string) => void
    title?: string
  },
): boolean {
  if (!plan.open) return false
  effects.openWindow(plan.conversationId, { title: effects.title })
  if (plan.closePaneId) effects.closePane(plan.closePaneId)
  return true
}
