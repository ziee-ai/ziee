import * as React from 'react'
import {
  Controller,
  FormProvider,
  useForm as useRhfForm,
  useFormContext,
  type FieldValues,
  type SubmitHandler,
  type UseFormProps,
  type UseFormReturn,
} from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Field, FieldLabel, FieldContent, FieldDescription, FieldError, FieldGroup } from '../shadcn/field'
import { KitSurfaceProvider } from './surface'
import { cn } from '@/lib/utils'

// Layout flows from Form → FormField via context (legacy layout / labelCol-width).
//   vertical (default) = label above control
//   horizontal         = label beside control (set `labelWidth` for a fixed label column)
//   inline             = fields flow left-to-right, wrapping
export type FormLayout = 'vertical' | 'horizontal' | 'inline'
interface FormLayoutValue {
  layout: FormLayout
  labelWidth?: number | string
}
const FormLayoutContext = React.createContext<FormLayoutValue>({ layout: 'vertical' })
const px = (w: number | string) => (typeof w === 'number' ? `${w}px` : w)

// react-hook-form is the engine. Validation = a zod schema via zodResolver:
//   const form = useForm<Values>({ resolver: zodResolver(schema), defaultValues })
// Kit default timing = 'onTouched' (validate after first blur, then re-validate on
// change). Override via `mode`/`reValidateMode`.
export { zodResolver }
export function useForm<T extends FieldValues>(props?: UseFormProps<T>): UseFormReturn<T> {
  return useRhfForm<T>({ mode: 'onTouched', ...props })
}

export interface FormProps<T extends FieldValues> {
  form: UseFormReturn<T>
  onSubmit: SubmitHandler<T>
  /** Disables every control inside (propagates via KitSurface). */
  disabled?: boolean
  size?: 'sm' | 'default' | 'lg'
  /** Native form name (legacy `name`); also used to namespace field ids if needed. */
  name?: string
  /** Label position for every FormField inside (legacy `layout`). Default 'vertical'. */
  layout?: FormLayout
  /** Fixed label-column width for horizontal layout (legacy labelCol), e.g. 120 or '8rem'. */
  labelWidth?: number | string
  className?: string
  children: React.ReactNode
}

export function Form<T extends FieldValues>({ form, onSubmit, disabled, size, name, layout = 'vertical', labelWidth, className, children }: FormProps<T>) {
  return (
    <FormProvider {...form}>
      <FormLayoutContext.Provider value={React.useMemo(() => ({ layout, labelWidth }), [layout, labelWidth])}>
        <KitSurfaceProvider disabled={disabled} size={size}>
          <form name={name} onSubmit={form.handleSubmit(onSubmit)} className={className} noValidate>
            {/* KitSurface disables kit components (+ <a>/custom); <fieldset disabled>
                also disables native + third-party form controls. `contents` keeps layout. */}
            <fieldset disabled={disabled} className="contents">
              {/* inline forms flow horizontally + wrap; the per-field <Field> is w-full by
                  default, so override it to w-auto here or every field claims a full row. */}
              <FieldGroup className={layout === 'inline' ? 'flex-row flex-wrap items-end gap-4 [&>[data-slot=field]]:w-auto' : undefined}>
                {children}
              </FieldGroup>
            </fieldset>
          </form>
        </KitSurfaceProvider>
      </FormLayoutContext.Provider>
    </FormProvider>
  )
}

export interface FormFieldProps {
  name: string
  label?: React.ReactNode
  description?: React.ReactNode
  className?: string
  /** Marks the field required: adds `aria-required` + a visual `*` on the label. */
  required?: boolean
  /** Prop the value binds to. Default 'value'; use 'checked' for Switch/Checkbox. */
  valuePropName?: string
  /**
   * A single kit control element. value/onChange/onBlur/name/id/ref + invalid +
   * aria-describedby are injected. The control MUST accept an `onChange(value)`
   * (kit controls do; Select aliases it). A consumer-supplied onChange/onBlur on the
   * child is composed (called after the form binding); a consumer ref is NOT merged.
   */
  children: React.ReactElement
}

// Wrap the control element; control comes from the Form context
// (no `control` prop). Bindings are injected via cloneElement onto the kit control.
export function FormField({ name, label, description, className, required, valuePropName = 'value', children }: FormFieldProps) {
  const { control } = useFormContext()
  const { layout, labelWidth } = React.useContext(FormLayoutContext)
  const beside = layout === 'horizontal' || layout === 'inline'
  const uid = React.useId()
  const fieldId = `${uid}-field`
  const descId = `${uid}-desc`
  const errId = `${uid}-err`
  const childProps = children.props as {
    onChange?: (...a: unknown[]) => void
    onBlur?: (...a: unknown[]) => void
  }
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState, formState }) => {
        // The zod resolver validates the WHOLE form each run, so errors[name] may be
        // populated for fields the user hasn't reached. Only SHOW a field's error once
        // it's been touched (or the form was submitted). `formState` here is the
        // SUBSCRIBED one from the render prop (the context copy doesn't re-render us).
        const showError = (fieldState.isTouched || formState.isSubmitted) && !!fieldState.error?.message
        const describedBy =
          [description ? descId : null, showError ? errId : null].filter(Boolean).join(' ') || undefined
        const injected: Record<string, unknown> = {
          [valuePropName]: field.value,
          // compose: run the form binding, then any consumer handler on the child.
          onChange: (...a: unknown[]) => {
            ;(field.onChange as (...x: unknown[]) => void)(...a)
            childProps.onChange?.(...a)
          },
          onBlur: (...a: unknown[]) => {
            field.onBlur()
            childProps.onBlur?.(...a)
          },
          name: field.name,
          id: fieldId,
          ref: field.ref,
          'aria-describedby': describedBy,
        }
        if (showError) injected.invalid = true
        if (required) injected['aria-required'] = true
        const labelEl = label != null && (
          <FieldLabel
            htmlFor={fieldId}
            // fixed label column for horizontal layout (legacy labelCol). Internal style
            // is allowed inside the kit; the style-guard only gates CONSUMER style props.
            style={beside && labelWidth != null ? { width: px(labelWidth), flex: 'none' } : undefined}
          >
            {label}
            {required && <span aria-hidden className="ml-0.5 text-destructive">*</span>}
          </FieldLabel>
        )
        const control = React.cloneElement(children, injected)
        const descEl = description != null && <FieldDescription id={descId}>{description}</FieldDescription>
        // FieldError already carries role="alert" → announced when it appears
        const errEl = showError && <FieldError id={errId}>{fieldState.error?.message}</FieldError>
        // horizontal/inline: label beside a FieldContent column (control + desc + error).
        if (beside) {
          return (
            <Field orientation="horizontal" data-invalid={showError || undefined} className={cn(className)}>
              {labelEl}
              <FieldContent>
                {control}
                {descEl}
                {errEl}
              </FieldContent>
            </Field>
          )
        }
        return (
          <Field data-invalid={showError || undefined} className={cn(className)}>
            {labelEl}
            {control}
            {descEl}
            {errEl}
          </Field>
        )
      }}
    />
  )
}
