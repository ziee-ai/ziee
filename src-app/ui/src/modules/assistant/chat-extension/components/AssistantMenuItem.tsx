import { Popover, theme } from 'antd'
import { RobotOutlined, RightOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'

/**
 * AssistantMenuItem Component
 * Menu item inside the + dropdown for selecting an assistant.
 * Opens a submenu to the right showing available assistants.
 */
export function AssistantMenuItem() {
  const { token } = theme.useToken()
  const { availableAssistants, selectedAssistantId, selectAssistant } =
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
          token={token}
          dividerAfter
        />
      )}
      {availableAssistants.length === 0 && (
        <div
          style={{
            padding: '6px 12px',
            fontSize: 13,
            color: token.colorTextSecondary,
          }}
        >
          No assistants available
        </div>
      )}
      {availableAssistants.map((assistant: any) => (
        <AssistantOption
          key={assistant.id}
          label={assistant.name}
          active={assistant.id === selectedAssistantId}
          onClick={() => handleSelect(assistant.id)}
          token={token}
        />
      ))}
    </div>
  )

  return (
    <Popover
      content={popoverContent}
      placement="rightTop"
      trigger={['hover', 'click']}
      arrow={false}
    >
      <div
        className="flex items-center justify-between gap-2 px-3 py-2 rounded-md cursor-pointer"
        style={{ color: token.colorTextBase, minWidth: 200 }}
        onMouseEnter={e => {
          e.currentTarget.style.backgroundColor = token.colorFillSecondary
        }}
        onMouseLeave={e => {
          e.currentTarget.style.backgroundColor = 'transparent'
        }}
      >
        <div className="flex items-center gap-2">
          <RobotOutlined style={{ fontSize: 16 }} />
          <span style={{ fontSize: 14 }}>
            {selectedAssistant ? selectedAssistant.name : 'Select assistant'}
          </span>
        </div>
        <RightOutlined style={{ fontSize: 10, opacity: 0.45 }} />
      </div>
    </Popover>
  )
}

function AssistantOption({
  label,
  active,
  onClick,
  token,
  dividerAfter,
}: {
  label: string
  active: boolean
  onClick: () => void
  token: ReturnType<typeof theme.useToken>['token']
  dividerAfter?: boolean
}) {
  return (
    <>
      <div
        role="button"
        tabIndex={0}
        aria-pressed={active}
        onClick={onClick}
        aria-current={active || undefined}
        onKeyDown={e => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            onClick()
          }
        }}
        className="cursor-pointer px-3 py-1.5 rounded-md focus-visible:outline focus-visible:outline-2"
        style={{
          fontSize: 14,
          backgroundColor: active ? token.colorPrimaryBg : 'transparent',
          color: active ? token.colorPrimary : token.colorTextBase,
        }}
        onMouseEnter={e => {
          if (!active)
            e.currentTarget.style.backgroundColor = token.colorFillSecondary
        }}
        onMouseLeave={e => {
          if (!active) e.currentTarget.style.backgroundColor = 'transparent'
        }}
        onFocus={e => {
          if (!active)
            e.currentTarget.style.backgroundColor = token.colorFillSecondary
        }}
        onBlur={e => {
          if (!active) e.currentTarget.style.backgroundColor = 'transparent'
        }}
      >
        {label}
      </div>
      {dividerAfter && (
        <div
          style={{
            height: 1,
            backgroundColor: token.colorBorderSecondary,
            margin: '4px 0',
          }}
        />
      )}
    </>
  )
}
