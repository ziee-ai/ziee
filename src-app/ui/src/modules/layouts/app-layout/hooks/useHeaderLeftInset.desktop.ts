import { isTauriView, isMacOS } from '@ziee/desktop/core/platform'
import { AppLayout } from '@/modules/layouts/app-layout/appLayout'

/**
 * DESKTOP override of {@link useHeaderLeftInset} — adds the macOS traffic-light
 * clearance. On macOS Tauri (sidebar collapsed, not fullscreen) the header must
 * clear BOTH the traffic-light cluster (ends ~x=72 after the x=20 shift) AND the
 * toggle button (marginLeft=84, width=28 → right edge ~112) → ~118px. Mirrors
 * `HeaderBarContainer.desktop`'s paddingLeft; kept in lockstep as the single
 * source of truth for the app header + the split leftmost pane header (ITEM-71).
 */
export function useHeaderLeftInset(): number {
  const { isSidebarCollapsed, isFullscreen } = AppLayout
  if (isSidebarCollapsed && isTauriView && !isFullscreen && isMacOS) return 118
  return isSidebarCollapsed ? 48 : 12
}
