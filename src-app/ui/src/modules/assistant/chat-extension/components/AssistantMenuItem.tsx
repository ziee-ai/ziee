import { Popover } from '@/components/ui'
import { Bot, ChevronRight } from 'lucide-react'
import { Stores } from '@/core/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'

/**
 * AssistantMenuItem Component
 * Menu item inside the + dropdown for selecting an assistant.
 * Opens a submenu to the right showing available assistants.
 */
export function AssistantMenuItem() {
  const { availableAssistants, selectedAssistantId, selectAssistant, loading } =
    Stores.AssistantPicker
  const { close } = usePlusDropdown()

  const selectedAssistant = availableAssistants.find(
    (a: any) => a.id === selectedAssistantId,
  )

  const handleSelect = (id: string | null) => {
    selectAssistant(id as any)
    close()
  }

  const popoverContent = (
    <div style={{ minWidth: 160, margin: -4 }}>
      {selectedAssistantId && (
        <AssistantOption
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
    >
      <div
        className="flex items-center justify-between gap-2 px-3 py-2 rounded-md cursor-pointer text-foreground min-w-[200px]"
        onMouseEnter={e => {
          e.currentTarget.className = 'flex items-center justify-between gap-2 px-3 py-2 rounded-md cursor-pointer text-foreground min-w-[200px] bg-muted'
        }}
        onMouseLeave={e => {
          e.currentTarget.className = 'flex items-center justify-between gap-2 px-3 py-2 rounded-md cursor-pointer text-foreground min-w-[200px]'
        }}
      >
        <div className="flex items-center gap-2">
          <Bot style={{ fontSize: 16 }} />
          <span style={{ fontSize: 14 }}>
            {loading && availableAssistants.length === 0
              ? 'Loading assistants…'
              : selectedAssistant
                ? selectedAssistant.name
                : 'Select assistant'}
          </span>
        </div>
        <ChevronRight style={{ fontSize: 10, opacity: 0.45 }} />
      </div>
    </Popover>
  )
}

function AssistantOption({
  label,
  active,
  onClick,
  dividerAfter,
}: {
  label: string
  active: boolean
  onClick: () => void
  dividerAfter?: boolean
}) {
  return (
    <>
      <div
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
