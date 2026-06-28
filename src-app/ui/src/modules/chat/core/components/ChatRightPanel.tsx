import { useEffect, useRef } from 'react'
import { Button, Empty, Tabs, Typography, theme } from 'antd'
import { CloseOutlined, ExclamationCircleOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { resolvePanelRenderer } from '@/modules/chat/core/stores/Chat.store'
import { ResizeHandle } from '@/modules/layouts/app-layout/components/ResizeHandle'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

const { Text } = Typography

function ActivePanelContent() {
  const { token } = theme.useToken()
  const { tabs, activeId } = Stores.Chat.rightPanel
  const activeTab = tabs.find(t => t.id === activeId)
  // Empty body when there's literally no tab to display — this is the
  // initial state and not an error, so returning null is correct here.
  if (!activeTab) return null

  const resolved = resolvePanelRenderer(activeTab)
  if (!resolved) {
    // The tab record exists but its type isn't registered — typically
    // means the owning extension hasn't been loaded (e.g., persisted tab
    // from a previous session whose extension is no longer registered).
    // Render an explicit message instead of silently going blank.
    return (
      <div className="flex flex-col items-center justify-center h-full p-6">
        <Empty
          image={<ExclamationCircleOutlined style={{ fontSize: 56, color: token.colorWarning }} />}
          description={
            <div className="flex flex-col items-center gap-1">
              <Text strong>Can't display this tab</Text>
              <Text type="secondary" className="text-xs">
                No renderer is registered for type{' '}
                <Text code className="!text-xs">{String(activeTab.type)}</Text>.
                The extension that owns this content may not be loaded.
              </Text>
            </div>
          }
        />
      </div>
    )
  }
  const { Component } = resolved
  // `data` is erased to ErasedPanelData at this render boundary; the precise
  // per-type shape was validated at the displayInRightPanel<T> call site.
  return <Component {...(activeTab.data as Record<string, unknown>)} />
}

function PanelTabs({ onCloseAll }: { onCloseAll: () => void }) {
  const { token } = theme.useToken()
  const { tabs, activeId } = Stores.Chat.rightPanel

  if (tabs.length === 0) return null

  return (
    <div
      className="flex-shrink-0"
      style={{ borderBottom: `1px solid ${token.colorBorderSecondary}` }}
      data-testid="chat-right-panel-tabs"
    >
      <Tabs
        type="editable-card"
        hideAdd
        size="small"
        activeKey={activeId ?? undefined}
        items={tabs.map(tab => {
          const resolved = resolvePanelRenderer(tab)
          return {
            key: tab.id,
            label: (
              <span className="flex items-center gap-1">
                {resolved?.icon}
                <span className="truncate max-w-[140px]">{tab.title}</span>
              </span>
            ),
            closable: true,
          }
        })}
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
              data-testid="chat-right-panel-close"
            />
          ),
        }}
      />
    </div>
  )
}

function handleDrawerKeyDown(
  e: React.KeyboardEvent,
  drawerRef: React.RefObject<HTMLDivElement | null>,
  onClose: () => void,
) {
  if (e.key === 'Escape') {
    e.stopPropagation()
    onClose()
    return
  }
  if (e.key === 'Tab' && drawerRef.current) {
    const focusable = drawerRef.current.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    )
    if (focusable.length === 0) return
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault()
      first.focus()
    }
  }
}

export function ChatRightPanel() {
  const panelRef = useRef<HTMLDivElement>(null)
  const drawerRef = useRef<HTMLDivElement>(null)
  const { token } = theme.useToken()
  const { rightPanel } = Stores.Chat
  const { sm: isMobile } = useWindowMinSize()

  const isOpen = rightPanel.tabs.length > 0 && rightPanel.activeId !== null
  const panelWidth = rightPanel.panelWidth
  const showDrawer = rightPanel.mobileDrawerOpen && rightPanel.tabs.length > 0 && rightPanel.activeId !== null

  // Mobile drawer: auto-focus the close button when opened for screen reader announcement
  useEffect(() => {
    if (showDrawer && drawerRef.current) {
      const closeBtn = drawerRef.current.querySelector<HTMLElement>('[data-testid="chat-right-panel-close"]')
      if (closeBtn) {
        closeBtn.focus()
      }
    }
  }, [showDrawer])

  // Mobile: full-screen fixed overlay so it covers the entire page including header
  if (isMobile) {
    if (!showDrawer) return null
    return (
      <div
        ref={drawerRef}
        className="fixed inset-0 z-[1000] flex flex-col"
        style={{ backgroundColor: token.colorBgLayout }}
        data-testid="chat-right-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Chat panel"
        onKeyDown={e => handleDrawerKeyDown(e, drawerRef, Stores.Chat.closeMobileDrawer)}
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
      data-testid="chat-right-panel"
      data-panel-open={isOpen ? 'true' : 'false'}
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
