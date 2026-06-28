import { useRef } from 'react'
import { Button, Empty, Tabs, Text } from '@/components/ui'
import { CircleAlert, X } from 'lucide-react'
import { Stores } from '@/core/stores'
import { resolvePanelRenderer } from '@/modules/chat/core/stores/Chat.store'
import { ResizeHandle } from '@/modules/layouts/app-layout/components/ResizeHandle'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

function ActivePanelContent() {
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
          data-testid="chat-panel-no-renderer-empty"
          description={
            <div className="flex flex-col items-center gap-1">
              <CircleAlert className="size-14 text-warning" />
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
  const { tabs, activeId } = Stores.Chat.rightPanel

  if (tabs.length === 0) return null

  return (
    <div
      className="flex-shrink-0 flex items-center border-b border-border"
      data-testid="chat-right-panel-tabs"
    >
      <Tabs
        data-testid="chat-right-panel-tab-list"
        editable
        hideAdd
        size="sm"
        className="flex-1 min-w-0"
        value={activeId ?? undefined}
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
        onValueChange={key => Stores.Chat.setActiveRightPanelTab(key)}
        onEdit={(action, key) => {
          if (action === 'remove') Stores.Chat.closeRightPanelTab(key)
        }}
      />
      <Button
        variant="ghost"
        size="sm"
        icon={<X className="size-3" />}
        className="!w-6 !h-6 !min-w-0 !p-0 opacity-60 hover:opacity-100 mr-1"
        title="Close panel"
        onClick={onCloseAll}
        data-testid="chat-right-panel-close"
        aria-label="Close panel"
      />
    </div>
  )
}

export function ChatRightPanel() {
  const panelRef = useRef<HTMLDivElement>(null)
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
        className="fixed inset-0 z-[1000] flex flex-col bg-background"
        data-testid="chat-right-panel"
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
      className={
        'relative flex-shrink-0 overflow-hidden transition-[width] duration-200 ease-in-out ' +
        (isOpen ? 'border-l border-border' : '')
      }
      style={{ width: isOpen ? panelWidth : 0 }}
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
