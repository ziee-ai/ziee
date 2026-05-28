/**
 * DELIBERATE DIVERGENCE from core's SidebarHeaderSpacer.
 *
 * Core renders a plain 50px spacer. Desktop wires it to the same
 * manual `startDragging()` mousedown handler used by
 * HeaderBarContainer — the spacer has no children, so a click here
 * is unambiguously a "drag the window" gesture (no buttons /
 * controls to compete with).
 *
 * Double-click toggles maximize, matching macOS titlebar behavior.
 *
 * Auto-detection via `data-tauri-drag-region` also works here
 * (immediate-target check passes since there are no children to
 * cover the spacer), but routing through the same manual API keeps
 * drag behavior consistent and trivially extensible (e.g., if we
 * ever overlay a control on this spacer, the INTERACTIVE_SEL
 * exemption already in the handler will protect it).
 */

import { useCallback } from 'react'
import { isTauriView, isLinux } from '@ziee/desktop/core/platform'
import { getCurrentWindow } from '@tauri-apps/api/window'

const INTERACTIVE_SEL =
  'button, a, input, textarea, select, [role="button"], [role="link"], [role="menuitem"], [role="combobox"], [contenteditable="true"], .ant-select, .ant-dropdown-trigger, .ant-segmented, .ant-segmented-item'

export function SidebarHeaderSpacer() {
  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isTauriView) return
      // Linux: WM-native chrome handles drag — see TauriDragRegion.tsx.
      if (isLinux) return
      if (e.button !== 0) return
      const target = e.target as Element
      if (target.closest?.(INTERACTIVE_SEL)) return
      e.preventDefault()
      void getCurrentWindow().startDragging()
    },
    [],
  )

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isTauriView) return
      if (isLinux) return
      const target = e.target as Element
      if (target.closest?.(INTERACTIVE_SEL)) return
      void getCurrentWindow().toggleMaximize()
    },
    [],
  )

  return (
    <div
      className="h-[50px] flex-shrink-0"
      onMouseDown={handleMouseDown}
      onDoubleClick={handleDoubleClick}
    />
  )
}
