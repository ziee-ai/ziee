import { Stores } from '@ziee/framework/stores'

/**
 * The header's LEFT padding (px) that reserves space for the fixed
 * `SidebarToggleButton` (the sidebar collapse/expand button, which floats over
 * the top-left when the sidebar is collapsed). SINGLE SOURCE OF TRUTH for the
 * app header (`HeaderBarContainer`) and the split view's leftmost per-pane header
 * (ITEM-71) — so the two never drift.
 *
 * Web / non-Tauri: 48 when collapsed (clears the toggle), 12 otherwise. The
 * `.desktop` override adds the macOS traffic-light clearance (118).
 */
export function useHeaderLeftInset(): number {
  const { isSidebarCollapsed } = Stores.AppLayout
  return isSidebarCollapsed ? 48 : 12
}
