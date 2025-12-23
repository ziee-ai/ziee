/**
 * Tauri Drag Region Component
 *
 * Creates a draggable region for window movement in Tauri apps.
 * Only renders in Tauri view, returns null in web browser.
 */

import { isTauriView } from '@ziee/desktop/core/platform'

interface TauriDragRegionProps extends React.HTMLAttributes<HTMLDivElement> {}

export const TauriDragRegion: React.FC<TauriDragRegionProps> = (props) => {
  if (!isTauriView) return null
  return <div data-tauri-drag-region="" {...props} />
}
