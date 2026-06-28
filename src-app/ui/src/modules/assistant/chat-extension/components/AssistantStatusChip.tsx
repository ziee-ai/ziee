import { Tag } from 'antd'
import { RobotOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

/**
 * AssistantStatusChip Component
 * Shows the selected assistant as a purple tag in the status row
 */
export function AssistantStatusChip() {
  const { selectedAssistantId, availableAssistants, clearAssistant } =
    Stores.AssistantPicker

  if (!selectedAssistantId) return null

  const assistant = availableAssistants.find(
    (a: any) => a.id === selectedAssistantId,
  )
  if (!assistant) return null

  return (
    <Tag
      color="purple"
      icon={<RobotOutlined />}
      closable
      onClose={() => clearAssistant()}
      style={{ margin: 0 }}
    >
      {assistant.name}
    </Tag>
  )
}
