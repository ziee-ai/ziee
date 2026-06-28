import { Select, Tooltip } from 'antd'
import { RobotOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

interface AssistantSelectorProps {
  disabled?: boolean
}

export function AssistantSelector({
  disabled = false,
}: AssistantSelectorProps) {
  // Access assistant store directly - reactive via store proxy
  const { availableAssistants, selectedAssistantId, selectAssistant } =
    Stores.AssistantPicker

  const handleChange = (assistantId: string) => {
    selectAssistant(assistantId)
  }

  // Build options for the select
  const options = availableAssistants.map((assistant: any) => ({
    label: assistant.name,
    value: assistant.id,
    title: assistant.description || assistant.name,
  }))

  // No assistants available: render a disabled, empty selector rather than
  // vanishing entirely, so the control stays present and self-explanatory.
  if (availableAssistants.length === 0) {
    return (
      <Tooltip title="No assistants available">
        <Select
          aria-label="Select Assistant"
          options={[]}
          disabled
          placeholder="No assistants"
          style={{ minWidth: 120 }}
          size="small"
          suffixIcon={<RobotOutlined />}
        />
      </Tooltip>
    )
  }

  return (
    <Tooltip title="Select Assistant">
      <Select
        aria-label="Select Assistant"
        value={selectedAssistantId}
        onChange={handleChange}
        options={options}
        disabled={disabled}
        placeholder="Assistant"
        style={{ minWidth: 120 }}
        size="small"
        suffixIcon={<RobotOutlined />}
      />
    </Tooltip>
  )
}
