import {
  DatePicker,
  FormField,
  Input,
  InputNumber,
  PasswordInput,
  Switch,
} from '@/components/ui'
import type { FieldSchema } from './elicitationOptions'

/**
 * Render a single NON-CHOICE elicitation field (boolean / number / date /
 * date-time / password / email / uri / text). Shared verbatim between the legacy
 * flat renderer (`ElicitationFormContent`) and the rich `AskUserWizardContent`
 * so the two never drift on input types + validation affordances. Choice fields
 * (enum / anyOf / oneOf, single or multi) are handled by each renderer directly
 * (Select in the legacy path, cards in the wizard).
 */
export function renderInputField(
  name: string,
  fieldSchema: FieldSchema,
  required: boolean,
): React.ReactNode {
  const label = fieldSchema.title || name
  const testId = `elicitation-field-${name}`

  if (fieldSchema.type === 'boolean') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        valuePropName="checked"
        description={fieldSchema.description}
      >
        <Switch data-testid={testId} />
      </FormField>
    )
  }

  if (fieldSchema.type === 'number' || fieldSchema.type === 'integer') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <InputNumber
          min={fieldSchema.minimum}
          max={fieldSchema.maximum}
          precision={fieldSchema.type === 'integer' ? 0 : undefined}
          className="w-full"
          data-testid={testId}
        />
      </FormField>
    )
  }

  // ─── String formats with dedicated pickers ─────────────────────────────
  // DatePicker stores an ISO string in form state; no dayjs conversion needed.
  // NOTE: the kit DatePicker is date-only (no showTime); date-time fields get
  // date-only selection and the time component will be T00:00:00 in the emitted
  // ISO string.
  if (fieldSchema.type === 'string' && fieldSchema.format === 'date') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <DatePicker
          placeholder={`Select ${label.toLowerCase()}`}
          aria-label={label}
          valueFormat="yyyy-MM-dd"
          className="w-full"
          data-testid={testId}
        />
      </FormField>
    )
  }

  if (fieldSchema.type === 'string' && fieldSchema.format === 'date-time') {
    // FLAG: kit DatePicker has no showTime — time will be T00:00:00 in the
    // emitted value. Full datetime picking requires a future kit component.
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <DatePicker
          placeholder={`Select ${label.toLowerCase()}`}
          aria-label={label}
          valueFormat="yyyy-MM-dd'T'HH:mm:ss"
          className="w-full"
          data-testid={testId}
        />
      </FormField>
    )
  }

  if (fieldSchema.format === 'password') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <PasswordInput showLabel="Show" hideLabel="Hide" data-testid={testId} />
      </FormField>
    )
  }

  const inputType =
    fieldSchema.format === 'email'
      ? 'email'
      : fieldSchema.format === 'uri'
        ? 'url'
        : 'text'

  return (
    <FormField
      key={name}
      name={name}
      label={label}
      required={required}
      description={fieldSchema.description}
    >
      <Input type={inputType} data-testid={testId} />
    </FormField>
  )
}
