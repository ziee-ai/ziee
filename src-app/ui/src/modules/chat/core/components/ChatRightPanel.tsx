import { useRef } from 'react'
import { Button, Empty, Tabs, Text } from '@/components/ui'
import { CircleAlert, X } from 'lucide-react'
import { Stores } from '@/core/stores'
import { resolvePanelRenderer } from '@/modules/chat/core/stores/Chat.store'
import { ResizeHandle } from '@/modules/layouts/app-layout/components/ResizeHandle'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'

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

function PanelTabs({ onCloseAll, asTitle = false }: { onCloseAll?: () => void; asTitle?: boolean }) {
  const { tabs, activeId } = Stores.Chat.rightPanel

  if (tabs.length === 0) return null

  const tabsEl = (
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
  )

  // In the Drawer the tabs ARE the header (the Drawer supplies the left back
  // button + chrome), so render just the tab strip — no border, no close button.
  if (asTitle) return tabsEl

  return (
    <div
      className="flex-shrink-0 flex items-center border-b border-border"
      data-testid="chat-right-panel-tabs"
    >
      {tabsEl}
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

export function ChatRightPanel({ narrow = false }: { narrow?: boolean }) {
  const panelRef = useRef<HTMLDivElement>(null)
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

  // Narrow page: the panel is an actual Drawer (full-width) — it handles the
  // backdrop, focus-trap, Escape, and swipe-to-close, and carries the
  // `data-slot="layout-drawer"` the sidebar-swipe guard keys on. The panel's own
  // PanelTabs is the header (no Drawer title), edge-to-edge (body padding zeroed).
  if (isMobile) {
    return (
      <Drawer
        open={showDrawer}
        onClose={Stores.Chat.closeMobileDrawer}
        placement="right"
        noBodyScrollWrap
        data-testid="chat-right-panel"
        // Tabs live in the title, beside the Drawer's own left back button.
        title={<PanelTabs asTitle />}
        titleText="File preview"
        // header pb-0: the tabs sit flush against the content (no gap), matching
        // the side-panel look. body: !p-0 edge-to-edge; overflow-hidden +
        // rounded-b-lg so the content clips to the drawer's rounded bottom corners.
        classNames={{ header: '!pb-0', body: '!p-0 overflow-hidden rounded-b-lg' }}
      >
        <div className="h-full overflow-hidden min-h-0">
          <ActivePanelContent />
        </div>
      </Drawer>
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
