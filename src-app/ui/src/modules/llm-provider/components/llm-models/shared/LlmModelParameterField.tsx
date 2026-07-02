import { useState } from 'react'
import {
  FormField,
  Input,
  PasswordInput,
  InputNumber,
  Select,
  Tag,
  Textarea,
} from '@/components/ui'

interface StringArrayInputProps {
  value?: string[]
  onChange?: (value: string[]) => void
  placeholder?: string
  className?: string
}

function StringArrayInput({
  value = [],
  onChange,
  placeholder,
  className,
}: StringArrayInputProps) {
  const [inputValue, setInputValue] = useState('')

  const handleAddTag = () => {
    if (inputValue.trim() && !value.includes(inputValue.trim())) {
      const newValue = [...value, inputValue.trim()]
      onChange?.(newValue)
      setInputValue('')
    }
  }

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      handleAddTag()
    }
  }

  return (
    <div className={className}>
      <div className="mb-2">
        {value?.map(tag => (
          <Tag variant="outline"
            key={tag}
            onClose={() => {
              const newValue = value.filter(t => t !== tag)
              onChange?.(newValue)
            }}
            closeLabel="Remove"
            className="mb-1"
            data-testid={`llm-string-array-tag-${tag}`}
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
        className="w-full"
        data-testid="llm-string-array-input"
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
}: LlmModelParameterFieldProps) {
  // Convert string[] name to dot-notation for react-hook-form
  const fieldName = Array.isArray(name) ? name.join('.') : name

  const renderInput = () => {
    switch (type) {
      case 'number':
        return (
          <InputNumber
            placeholder={placeholder}
            className="w-full"
            min={min}
            max={max}
            step={step}
            data-testid={`llm-param-${fieldName}`}
          />
        )
      case 'password':
        return <PasswordInput showLabel="Show" hideLabel="Hide" placeholder={placeholder} className="w-full" data-testid={`llm-param-${fieldName}`} />
      case 'textarea':
        return <Textarea placeholder={placeholder} rows={3} data-testid={`llm-param-${fieldName}`} />
      case 'select':
        return (
          <Select
            placeholder={placeholder}
            className="w-full"
            options={(options ?? []).map(o => ({ value: String(o.value), label: o.label }))}
            data-testid={`llm-param-${fieldName}`}
          />
        )
      case 'string-array':
        return (
          <StringArrayInput placeholder={placeholder} className="w-full" />
        )
      case 'text':
      default:
        return <Input placeholder={placeholder} className="w-full" data-testid={`llm-param-${fieldName}`} />
    }
  }

  return (
    <FormField
      name={fieldName}
      label={label}
      description={help}
      required={required}
      valuePropName={type === 'string-array' ? 'value' : undefined}
    >
      {renderInput()}
    </FormField>
  )
}
