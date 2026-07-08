import { useEffect, useRef } from 'react'
import { Button, Empty, Tabs, Text } from '@/components/ui'
import { CircleAlert, X } from 'lucide-react'
import { Stores } from '@/core/stores'
import { resolvePanelRenderer } from '@/modules/chat/core/stores/Chat.store'
import { ResizeHandle } from '@/modules/layouts/app-layout/components/ResizeHandle'

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
        // Quiet UNDERLINE tabs (J5): a boxed/segmented strip reads as heavy in this
        // narrow side panel — `line` drops the box/fill and marks the active tab with
        // an underline bar + stronger text, inactive tabs are muted text.
        variant="line"
        // Single-row HORIZONTAL scroll only (I5): DivScrollX is overflow-x auto +
        // overflow-y hidden with the app's overlay scrollbar, and its inner row is
        // `items-center` so the tabs are vertically centered in the strip (A8).
        scrollX
        // !gap-0: the kit Tabs root is `flex flex-col gap-2`, which separates the
        // strip from its content panels. We render the strip ONLY (the panel body
        // is drawn separately, so the TabsContent panels are empty) — that 8px gap
        // otherwise shows as dead space below the tabs AND pushes the strip above
        // the items-center close button, so the tabs and the × misalign.
        className="flex-1 min-w-0 !gap-0"
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
        size="default"
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

export function ChatRightPanel({ narrow = false }: { narrow?: boolean }) {
  const panelRef = useRef<HTMLDivElement>(null)
  const drawerRef = useRef<HTMLDivElement>(null)
  const { rightPanel } = Stores.Chat
  // `narrow` = the conversation PAGE is small (element-width, sidebar-aware),
  // not the window — so an open sidebar on a wide window still gets the drawer.
  const isMobile = narrow

  const isOpen = rightPanel.tabs.length > 0 && rightPanel.activeId !== null
  const panelWidth = rightPanel.panelWidth
  const showDrawer =
    rightPanel.mobileDrawerOpen &&
    rightPanel.tabs.length > 0 &&
    rightPanel.activeId !== null

  // Mobile drawer: auto-focus the close button when opened (screen-reader
  // announcement) and restore focus to the opener when it closes.
  const previouslyFocusedRef = useRef<HTMLElement | null>(null)
  useEffect(() => {
    if (showDrawer && drawerRef.current) {
      previouslyFocusedRef.current = document.activeElement as HTMLElement | null
      const closeBtn = drawerRef.current.querySelector<HTMLElement>(
        '[data-testid="chat-right-panel-close"]',
      )
      if (closeBtn) {
        closeBtn.focus()
      }
    } else if (!showDrawer && previouslyFocusedRef.current) {
      previouslyFocusedRef.current.focus?.()
      previouslyFocusedRef.current = null
    }
  }, [showDrawer])

  // Mobile: full-screen fixed overlay so it covers the entire page including header
  if (isMobile) {
    if (!showDrawer) return null
    return (
      <div
        ref={drawerRef}
        className="fixed inset-0 z-[1000] flex flex-col bg-background"
        data-testid="chat-right-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Chat panel"
        onKeyDown={e =>
          handleDrawerKeyDown(e, drawerRef, Stores.Chat.closeMobileDrawer)
        }
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
