import { Button, theme, Tooltip, Typography } from 'antd'
import { EditOutlined, CloseOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Shows a banner above the Chat Input when the user is in edit mode.
 * Displays a "Editing message" label and a Cancel button.
 *
 * Rendered by ChatInput whenever Stores.Chat.editingMessage is non-null.
 * Clicking Cancel calls cancelEdit() which clears editingMessage, restores
 * trimmed messages, and clears the text input.
 */
export function EditingMessageBanner() {
  const editingMessage = Stores.Chat.editingMessage
  const { token } = theme.useToken()

  if (!editingMessage) return null

  return (
    <div
      className="flex items-center justify-between px-3 py-1.5"
      style={{
        borderBottom: `1px solid ${token.colorBorderSecondary}`,
        backgroundColor: token.colorFillQuaternary,
        borderRadius: `${token.borderRadiusLG}px ${token.borderRadiusLG}px 0 0`,
      }}
    >
      <div className="flex items-center gap-1.5">
        <EditOutlined style={{ fontSize: 12, color: token.colorTextSecondary }} />
        <Text type="secondary" className="text-xs">
          Editing message
        </Text>
      </div>
      <Tooltip title="Cancel edit">
        <Button
          type="text"
          size="small"
          icon={<CloseOutlined />}
          onClick={() => Stores.Chat.__state.cancelEdit()}
          aria-label="Cancel edit"
        />
      </Tooltip>
    </div>
  )
}
