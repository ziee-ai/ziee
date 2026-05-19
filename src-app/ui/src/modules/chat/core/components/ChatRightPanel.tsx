import { useRef } from 'react'
import { Button, Tabs, theme } from 'antd'
import { CloseOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { ResizeHandle } from '@/modules/layouts/app-layout/components/ResizeHandle'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

function ActivePanelContent() {
  const { tabs, activeId } = Stores.Chat.rightPanel
  const activeTab = tabs.find(t => t.id === activeId)
  if (!activeTab) return null
  const Component = activeTab.component
  return <Component />
}

function PanelTabs({ onCloseAll }: { onCloseAll: () => void }) {
  const { token } = theme.useToken()
  const { tabs, activeId } = Stores.Chat.rightPanel

  if (tabs.length === 0) return null

  return (
    <div
      className="flex-shrink-0"
      style={{ borderBottom: `1px solid ${token.colorBorderSecondary}` }}
    >
      <Tabs
        type="editable-card"
        hideAdd
        size="small"
        activeKey={activeId ?? undefined}
        items={tabs.map(tab => ({
          key: tab.id,
          label: (
            <span className="flex items-center gap-1">
              {tab.icon}
              <span className="truncate max-w-[140px]">{tab.title}</span>
            </span>
          ),
          closable: true,
        }))}
        onChange={key => Stores.Chat.setActiveRightPanelTab(key)}
        onEdit={(key, action) => {
          if (action === 'remove') Stores.Chat.closeRightPanelTab(key as string)
        }}
        tabBarExtraContent={{
          right: (
            <Button
              type="text"
              size="small"
              icon={<CloseOutlined style={{ fontSize: 12 }} />}
              className="!w-6 !h-6 !min-w-0 !p-0 opacity-60 hover:opacity-100 mr-1"
              title="Close panel"
              onClick={onCloseAll}
            />
          ),
        }}
      />
    </div>
  )
}

export function ChatRightPanel() {
  const panelRef = useRef<HTMLDivElement>(null)
  const { token } = theme.useToken()
  const { rightPanel } = Stores.Chat
  const { sm: isMobile } = useWindowMinSize()

  const isOpen = rightPanel.tabs.length > 0 && rightPanel.activeId !== null
  const panelWidth = rightPanel.panelWidth

  // Mobile: full-screen fixed overlay so it covers the entire page including header
  if (isMobile) {
    const showDrawer = rightPanel.mobileDrawerOpen && rightPanel.tabs.length > 0 && rightPanel.activeId !== null
    if (!showDrawer) return null
    return (
      <div
        className="fixed inset-0 z-[1000] flex flex-col"
        style={{ backgroundColor: token.colorBgLayout }}
      >
        <PanelTabs onCloseAll={Stores.Chat.closeMobileDrawer} />
        <div className="flex-1 overflow-hidden">
          <ActivePanelContent />
        </div>
      </div>
    )
  }

  // Desktop: resizable side panel
  return (
    <div
      ref={panelRef}
      className="relative flex-shrink-0 overflow-hidden transition-[width] duration-200 ease-in-out"
      style={{
        width: isOpen ? panelWidth : 0,
        borderLeft: isOpen ? `1px solid ${token.colorBorderSecondary}` : undefined,
      }}
    >
      {/* Inner div keeps fixed width so content doesn't collapse during close animation */}
      <div className="h-full flex flex-col" style={{ width: panelWidth, minWidth: panelWidth }}>
        <PanelTabs onCloseAll={Stores.Chat.closeAllRightPanelTabs} />
        <div className="flex-1 overflow-hidden">
          <ActivePanelContent />
        </div>
      </div>
      {isOpen && (
        <ResizeHandle
          placement="left"
          parentLevel={0}
          minWidth={240}
          maxWidth={800}
          onEnd={() => {
            if (panelRef.current) {
              Stores.Chat.setRightPanelWidth(panelRef.current.offsetWidth)
            }
          }}
        />
      )}
    </div>
  )
}
