import React from 'react'
import { AppLayout as ShellAppLayout } from '@ziee/shell/layouts/AppLayout'
import { LeftSidebar } from '@/modules/layouts/app-layout/components/LeftSidebar'
import { SidebarToggleButton } from '@/modules/layouts/app-layout/components/SidebarToggleButton'
import { usePopoutSnapBackListener } from '@/modules/chat/core/popout/usePopoutSnapBack'

/**
 * AppLayout — the ziee app's INJECTION SITE for the generic shell layout.
 *
 * The layout structure (sidebar chrome + the 7 slot definitions + drag/touch
 * behavior) lives in `@ziee/shell`. The two platform-variant leaves it renders
 * — `LeftSidebar` + `SidebarToggleButton` — carry app-side `.desktop.tsx`
 * variants. They are imported here via `@/`-prefixed specifiers so the desktop
 * `localOverridePlugin` swaps them to their `.desktop` variants at THIS site
 * (the web build resolves the base files). This keeps the existing whole-file
 * override mechanism intact even though the shell package uses relative imports
 * internally.
 */
export function AppLayout({ children }: { children: React.ReactNode }) {
  // MAIN-window only (the layout-less /chat-window pop-out route does NOT render
  // AppLayout): snap a closing pop-out window's conversation back in as a pane
  // (ITEM-54). App-specific, so it lives at this injection site rather than in
  // the generic shell layout. No-op on web.
  usePopoutSnapBackListener()
  return (
    <ShellAppLayout
      LeftSidebar={LeftSidebar}
      SidebarToggleButton={SidebarToggleButton}
    >
      {children}
    </ShellAppLayout>
  )
}

export default AppLayout
