import { Form, Select, Button } from 'antd'
import { SettingOutlined } from '@ant-design/icons'
import { IoIosArrowDown } from 'react-icons/io'

interface ModelSelectorProps {
  isBreaking: boolean
  isDisabled: boolean
  availableModels: Array<{
    label: string
    options: Array<{ label: string; value: string; description?: string }>
  }>
}

export function ModelSelector({
  isBreaking,
  isDisabled,
  availableModels,
}: ModelSelectorProps) {
  return (
    <Form.Item
      name="model"
      label="Model"
      className="mb-0"
      style={{ display: 'inline-block' }}
    >
      <Select
        popupMatchSelectWidth={false}
        placeholder="Model"
        disabled={isDisabled}
        options={availableModels}
        style={{ width: isBreaking ? 40 : 120 }}
        variant={isBreaking ? 'borderless' : undefined}
        labelRender={isBreaking ? () => '' : undefined}
        prefix={
          isBreaking && (
            <Button>
              <SettingOutlined />
            </Button>
          )
        }
        suffixIcon={<IoIosArrowDown />}
      />
    </Form.Item>
  )
}
