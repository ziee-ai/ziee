import { Button } from 'antd'
import { GoSidebarCollapse, GoSidebarExpand } from 'react-icons/go'
import { Stores } from '@/core/stores'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

export function SidebarToggleButton() {
  const { isSidebarCollapsed } = Stores.AppLayout
  const windowMinSize = useWindowMinSize()

  // Mobile (windowMinSize.xs = viewport ≤ 480px): use a 44×44 tap target
  // per WCAG 2.5.5 (Target Size — Minimum). Desktop keeps the compact
  // 24px chevron next to the sidebar edge. (audit 02 R-1)
  const isMobile = windowMinSize.xs
  const dimension = isMobile ? '44px' : '24px'
  const iconFontSize = isMobile ? '24px' : '30px'

  return (
    <div
      className="flex items-center gap-6 mr-4 fixed z-10 h-[50px]"
      style={{
        left: 12,
        top: 0,
      }}
    >
      <Button
        type="text"
        onClick={Stores.AppLayout.toggleSidebar}
        className="flex items-center justify-center"
        style={{
          width: dimension,
          height: dimension,
          padding: 0,
          fontSize: iconFontSize,
          borderRadius: '4px',
          minWidth: dimension,
        }}
        aria-label={isSidebarCollapsed ? 'Open navigation menu' : 'Close navigation menu'}
        aria-expanded={!isSidebarCollapsed}
        aria-controls="app-sidebar"
      >
        {isSidebarCollapsed ? (
          <GoSidebarCollapse aria-hidden="true" />
        ) : (
          <GoSidebarExpand aria-hidden="true" />
        )}
      </Button>
    </div>
  )
}
