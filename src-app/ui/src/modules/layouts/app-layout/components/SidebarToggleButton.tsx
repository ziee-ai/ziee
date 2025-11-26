import { Button } from 'antd'
import { GoSidebarCollapse, GoSidebarExpand } from 'react-icons/go'
import { Stores } from '@/core/stores'

export function SidebarToggleButton() {
  const { isSidebarCollapsed } = Stores.AppLayout

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
          width: '24px',
          height: '24px',
          padding: 0,
          fontSize: '30px',
          borderRadius: '4px',
          minWidth: '20px',
        }}
        aria-label={isSidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
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
