import { Popover } from '@ziee/kit'
import { Bot, ChevronRight } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { newChatAssistantKey } from '@/modules/assistant/stores'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'

/**
 * AssistantMenuItem Component
 * Menu item inside the + dropdown for selecting an assistant.
 * Opens a submenu to the right showing available assistants.
 */
export function AssistantMenuItem() {
  // Permission gate (layer 4) — mirrors KbMenuItem. Without `assistants::read`
  // the picker's store never loads anything (it self-gates), so an ungated menu
  // item would render forever as a dead end ("No assistants available") for a
  // user who also has no Settings -> Assistants page to populate it from.
  const canRead = usePermission(Permissions.AssistantsRead)
  // Per-conversation selection (ITEM-5): the picker store keys the selected
  // assistant by conversation/pane, so `selectedAssistantId` is derived below
  // from `selectedByConversation[key]`, not read globally off the store.
  const { availableAssistants, selectedByConversation, selectAssistant, clearAssistant, loading } =
    Stores.AssistantPicker
  const { close } = usePlusDropdown()
  // Key by THIS pane's conversation (bridge-resolved). (ITEM-5)
  const pane = useChatPaneOrNull()
  const key =
    Stores.Chat.conversation?.id ?? newChatAssistantKey(pane?.paneId)
  const selectedAssistantId = selectedByConversation[key]

  const selectedAssistant = availableAssistants.find(
    (a: any) => a.id === selectedAssistantId,
  )

  if (!canRead) return null

  const handleSelect = (id: string | null) => {
    if (id) selectAssistant(key, id)
    else clearAssistant(key)
    close()
  }

  const popoverContent = (
    <div data-testid="assistant-menu-options" style={{ minWidth: 160, margin: -4 }}>
      {selectedAssistantId && (
        <AssistantOption
          testid="assistant-option-none"
          label="No assistant"
          active={false}
          onClick={() => handleSelect(null)}
          dividerAfter
        />
      )}
      {availableAssistants.length === 0 && (
        <div className="px-3 py-1.5 text-sm text-muted-foreground">
          No assistants available
        </div>
      )}
      {availableAssistants.map((assistant: any) => (
        <AssistantOption
          key={assistant.id}
          testid={`assistant-option-${assistant.id}`}
          label={assistant.name}
          active={assistant.id === selectedAssistantId}
          onClick={() => handleSelect(assistant.id)}
        />
      ))}
    </div>
  )

  return (
    <Popover
      content={popoverContent}
      side="right"
      align="start"
      className="w-auto"
    >
      <div
        data-testid="assistant-menu-trigger"
        className="flex items-center gap-2 px-3 py-1.5 rounded-md cursor-pointer text-foreground hover:bg-muted whitespace-nowrap"
      >
        <div className="flex min-w-0 items-center gap-2">
          <Bot className="size-4 shrink-0" />
          <span className="min-w-0 flex-1 truncate text-sm">
            {loading && availableAssistants.length === 0
              ? 'Loading assistants…'
              : selectedAssistant
                ? selectedAssistant.name
                : 'Select assistant'}
          </span>
        </div>
        <ChevronRight className="size-3 shrink-0 opacity-45" />
      </div>
    </Popover>
  )
}

function AssistantOption({
  label,
  active,
  onClick,
  dividerAfter,
  testid,
}: {
  label: string
  active: boolean
  onClick: () => void
  dividerAfter?: boolean
  testid?: string
}) {
  return (
    <>
      <div
        data-testid={testid}
        role="button"
        tabIndex={0}
        aria-pressed={active}
        aria-current={active || undefined}
        onClick={onClick}
        onKeyDown={e => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            onClick()
          }
        }}
        className={`cursor-pointer px-3 py-1.5 rounded-md text-sm focus-visible:outline focus-visible:outline-2 ${active ? 'bg-accent text-primary' : 'text-foreground'}`}
        onMouseEnter={e => {
          if (!active)
            e.currentTarget.className = 'cursor-pointer px-3 py-1.5 rounded-md text-sm focus-visible:outline focus-visible:outline-2 text-foreground bg-muted'
        }}
        onMouseLeave={e => {
          if (!active) e.currentTarget.className = 'cursor-pointer px-3 py-1.5 rounded-md text-sm focus-visible:outline focus-visible:outline-2 text-foreground'
        }}
        onFocus={e => {
          if (!active)
            e.currentTarget.className = 'cursor-pointer px-3 py-1.5 rounded-md text-sm focus-visible:outline focus-visible:outline-2 text-foreground bg-muted'
        }}
        onBlur={e => {
          if (!active) e.currentTarget.className = 'cursor-pointer px-3 py-1.5 rounded-md text-sm focus-visible:outline focus-visible:outline-2 text-foreground'
        }}
      >
        {label}
      </div>
      {dividerAfter && (
        <div className="h-px bg-border my-1" />
      )}
    </>
  )
}
