/**
 * Desktop override for seam `layout.sidebar-header-spacer`.
 *
 * Core renders a plain 50px spacer; the desktop wires it to the same manual
 * `startDragging()` mousedown used by HeaderBarContainer — the spacer has no
 * children, so a click here is unambiguously a "drag the window" gesture.
 * Double-click toggles maximize (macOS titlebar behavior). The
 * `INTERACTIVE_SEL` exemption future-proofs against overlaid controls.
 */
import { useCallback } from 'react'
import { registerOverride } from '@/core/overrides'
import { isTauriView, isLinux } from '@ziee/desktop/core/platform'
import { getCurrentWindow } from '@tauri-apps/api/window'

const INTERACTIVE_SEL =
  'button, a, input, textarea, select, [role="button"], [role="link"], [role="menuitem"], [role="combobox"], [contenteditable="true"]'

function DesktopSidebarHeaderSpacer() {
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

export function register(): void {
  registerOverride('layout.sidebar-header-spacer', DesktopSidebarHeaderSpacer)
}
