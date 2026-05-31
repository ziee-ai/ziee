/**
 * Tauri Drag Region Component
 *
 * Creates a draggable region for window movement in Tauri apps.
 * No-op in the web browser AND on Linux: Linux ships with `decorations:
 * true` (native WM title bar drawn OUTSIDE the webview), so the WM
 * already handles window-drag — adding an in-app drag region on top of
 * the header would be redundant and could eat clicks on header
 * controls.
 */

import { isTauriView, isLinux } from '@ziee/desktop/core/platform'

interface TauriDragRegionProps extends React.HTMLAttributes<HTMLDivElement> {}

export const TauriDragRegion: React.FC<TauriDragRegionProps> = (props) => {
  if (!isTauriView || isLinux) return null
  return <div data-tauri-drag-region="" {...props} />
}
