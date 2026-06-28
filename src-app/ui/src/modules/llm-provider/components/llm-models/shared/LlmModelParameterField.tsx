import {
  FormField,
  Input,
  InputNumber,
  MultiSelect,
  PasswordInput,
  Select,
  Textarea,
} from '@/components/ui'

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
  const fieldName = Array.isArray(name) ? name.join('.') : name

  const renderInput = () => {
    switch (type) {
      case 'number':
        return (
          <InputNumber
            placeholder={placeholder}
            min={min}
            max={max}
            step={step}
          />
        )
      case 'password':
        return (
          <PasswordInput
            placeholder={placeholder}
            showLabel="Show"
            hideLabel="Hide"
          />
        )
      case 'textarea':
        return <Textarea placeholder={placeholder} rows={3} />
      case 'select':
        return (
          <Select
            placeholder={placeholder}
            options={(options ?? []).map(o => ({
              value: String(o.value),
              label: o.label,
            }))}
          />
        )
      case 'string-array':
        return (
          <MultiSelect
            allowCreate
            options={[]}
            placeholder={placeholder ?? ''}
            searchPlaceholder="Type to add"
            emptyText="No values"
            removeLabel={l => `Remove ${l}`}
            aria-label={label}
          />
        )
      case 'text':
      default:
        return <Input placeholder={placeholder} />
    }
  }

  return (
    <FormField
      name={fieldName}
      label={label}
      description={help}
      required={required}
    >
      {renderInput()}
    </FormField>
  )
}
