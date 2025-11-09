import { Form, Input, InputNumber, Select, Tag } from 'antd'
import { useState } from 'react'

const { TextArea } = Input

interface StringArrayInputProps {
  value?: string[]
  onChange?: (value: string[]) => void
  placeholder?: string
  style?: React.CSSProperties
}

function StringArrayInput({
  value = [],
  onChange,
  placeholder,
  style,
}: StringArrayInputProps) {
  const [inputValue, setInputValue] = useState('')

  const handleAddTag = () => {
    if (inputValue.trim() && !value.includes(inputValue.trim())) {
      const newValue = [...value, inputValue.trim()]
      onChange?.(newValue)
      setInputValue('')
    }
  }

  const handleRemoveTag = (tagToRemove: string) => {
    const newValue = value.filter(tag => tag !== tagToRemove)
    onChange?.(newValue)
  }

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      handleAddTag()
    }
  }

  return (
    <div style={style}>
      <div style={{ marginBottom: 8 }}>
        {value?.map(tag => (
          <Tag
            key={tag}
            closable
            onClose={() => handleRemoveTag(tag)}
            style={{ marginBottom: 4 }}
          >
            {tag}
          </Tag>
        ))}
      </div>
      <Input
        value={inputValue}
        onChange={e => setInputValue(e.target.value)}
        onKeyPress={handleKeyPress}
        onBlur={handleAddTag}
        placeholder={placeholder || 'Press Enter to add'}
        style={{ width: '100%' }}
      />
    </div>
  )
}

export interface ParameterFieldConfig {
  name: string | string[]
  label: string
  help?: string
  placeholder?: string
  type: 'number' | 'text' | 'password' | 'textarea' | 'select' | 'string-array'
  min?: number
  max?: number
  step?: number
  required?: boolean
  options?: Array<{ value: string | number; label: string }>
  rules?: any[]
}

type LlmModelParameterFieldProps = ParameterFieldConfig

export function LlmModelParameterField({
  name,
  label,
  help,
  placeholder,
  type,
  min,
  max,
  step,
  required,
  options,
  rules = [],
}: LlmModelParameterFieldProps) {
  const fieldRules = [
    ...(required ? [{ required: true, message: `${label} is required` }] : []),
    ...rules,
  ]

  const renderInput = () => {
    const commonStyle = { width: '100%' }

    switch (type) {
      case 'number':
        return (
          <InputNumber
            placeholder={placeholder}
            style={commonStyle}
            min={min}
            max={max}
            step={step}
          />
        )
      case 'password':
        return <Input.Password placeholder={placeholder} style={commonStyle} />
      case 'textarea':
        return <TextArea placeholder={placeholder} rows={3} />
      case 'select':
        return (
          <Select
            placeholder={placeholder}
            style={commonStyle}
            options={options}
          />
        )
      case 'string-array':
        return (
          <StringArrayInput placeholder={placeholder} style={commonStyle} />
        )
      case 'text':
      default:
        return <Input placeholder={placeholder} style={commonStyle} />
    }
  }

  return (
    <Form.Item name={name} label={label} tooltip={help} rules={fieldRules}>
      {renderInput()}
    </Form.Item>
  )
}
