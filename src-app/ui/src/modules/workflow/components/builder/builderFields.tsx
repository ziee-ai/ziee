import {
  type ReactElement,
  type ReactNode,
  cloneElement,
  isValidElement,
  useId,
} from 'react'
import { Text, Textarea } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { RefInsertMenu } from './RefInsertMenu'

// ---------------------------------------------------------------------------
// Small shared building blocks for the per-kind step forms. The builder edits
// live store state on every keystroke (not a submit-once form), so these mirror
// the plain label+control pattern used elsewhere for non-RHF controls (e.g. the
// model Select in WorkflowRunDialog) rather than the RHF `FormField` wrapper.
// ---------------------------------------------------------------------------

interface LabeledControlProps {
  label: ReactNode
  htmlFor?: string
  required?: boolean
  description?: ReactNode
  error?: string | null
  /** Right-aligned action rendered on the label row (e.g. a RefInsertMenu). */
  action?: ReactNode
  children: ReactNode
}

export function LabeledControl({
  label,
  htmlFor,
  required,
  description,
  error,
  action,
  children,
}: LabeledControlProps) {
  // Associate the label with its control so every control rendered through
  // LabeledControl — including the bare `InputNumber`s (agent max-steps,
  // sandbox/elicit timeout, llm_map parallel/retries) that carry no aria-label
  // of their own — has an accessible NAME. We inject a generated id onto the
  // single child (kit controls forward `id`/`...props` to the underlying DOM
  // control) and point the label's `htmlFor` at it. An explicit `htmlFor` prop
  // still wins; a child that already sets its own `id` is left untouched.
  const generatedId = useId()
  const controlId = htmlFor ?? generatedId
  const control =
    isValidElement(children) &&
    (children.props as { id?: string }).id == null
      ? cloneElement(children as ReactElement<{ id?: string }>, {
          id: controlId,
        })
      : children
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between gap-2 min-h-6">
        <label htmlFor={controlId} className="text-xs font-medium">
          {label}
          {required && <span className="text-destructive"> *</span>}
        </label>
        {action}
      </div>
      {control}
      {description && (
        <Text type="secondary" className="text-xs">
          {description}
        </Text>
      )}
      {error && <span className="text-xs text-destructive">{error}</span>}
    </div>
  )
}

interface PromptFieldProps {
  store: WorkflowBuilderStore
  stepId: string
  label: ReactNode
  value: string
  onChange: (value: string) => void
  placeholder?: string
  rows?: number
  required?: boolean
  description?: ReactNode
  error?: string | null
  testid: string
}

/** A template-aware textarea: a RefInsertMenu on the label row appends the
 *  chosen `{{ … }}` reference to the field. */
export function PromptField({
  store,
  stepId,
  label,
  value,
  onChange,
  placeholder,
  rows = 5,
  required,
  description,
  error,
  testid,
}: PromptFieldProps) {
  return (
    <LabeledControl
      label={label}
      required={required}
      description={description}
      error={error}
      action={
        <RefInsertMenu
          store={store}
          stepId={stepId}
          onInsert={token => onChange(value ? `${value}${token}` : token)}
          testid={`${testid}-ref`}
        />
      }
    >
      <Textarea
        data-testid={testid}
        rows={rows}
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
      />
    </LabeledControl>
  )
}
