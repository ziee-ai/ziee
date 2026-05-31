/**
 * Top spacer in the LeftSidebar that clears the area covered by
 * SidebarToggleButton (fixed-positioned over the top 50px).
 *
 * Core renders a plain 50px-tall div. Desktop's vite
 * localOverridePlugin swaps this for a version that wires the
 * spacer to Tauri window-drag via mousedown, so the user can drag
 * the window from the empty area above the sidebar nav items.
 */
export function SidebarHeaderSpacer() {
  return <div className="h-[50px] flex-shrink-0" />
}
