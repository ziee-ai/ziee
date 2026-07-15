/**
 * Whether the "Open in new tab / window" (pop-out) header action should render
 * (ITEM-44, DEC-60 / FB-9).
 *
 * - Inside a SPLIT pane it ALWAYS shows (both platforms) — there it is the
 *   "move this pane out into its own window" action.
 * - In SINGLE-pane it shows ONLY on the desktop app: a native OS window is the
 *   only way to get a second top-level view there, so the button is the sole
 *   affordance. On the WEB, the browser already provides "open in new tab"
 *   (Cmd/middle-click, duplicate tab), so an in-app single-pane button is
 *   redundant chrome and is hidden.
 * - NEVER inside a pop-out WINDOW itself (ITEM-56 / FB-13): "open in new window"
 *   inside the window that already IS the pop-out is a no-op that just focuses
 *   itself (dedup by the `chat-<id>` label), i.e. confusing chrome. The pop-out
 *   window is a focused single-conversation view, so it hides window-management
 *   actions (back / split / pop-out).
 *
 * Pure — no window/DOM access; the caller passes the facts so this is
 * unit-testable without a Tauri/DOM environment (mirrors `needsOpenChoice`).
 */
export function popoutActionVisible(
  inPane: boolean,
  isDesktop: boolean,
  isPopoutWindow = false,
): boolean {
  if (isPopoutWindow) return false
  return inPane || isDesktop
}
