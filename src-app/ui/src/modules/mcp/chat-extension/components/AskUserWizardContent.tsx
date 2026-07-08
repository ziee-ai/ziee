import { useEffect, useId, useMemo, useRef, useState } from 'react'
import {
  Badge,
  Button,
  Card,
  Checkbox,
  Controller,
  Form,
  Input,
  Text,
  useForm,
  zodResolver,
  type UseFormReturn,
} from '@/components/ui'
import { RadioGroup, RadioGroupItem } from '@/components/ui/shadcn/radio-group'
import { SquarePen } from 'lucide-react'
import { Stores } from '@/core/stores'
import {
  allowsOther,
  buildFormSchema,
  finalizeValues,
  getRichOptions,
  isMultiChoiceField,
  isOtherSelected,
  isSingleChoiceField,
  orderRecommendedFirst,
  otherFieldError,
  OTHER_SENTINEL,
  type FieldSchema,
  type RichOption,
} from './elicitationOptions'
import { renderInputField } from './elicitationFields'

interface AskUserWizardContentProps {
  elicitationId: string
  message: string
  server: string
  properties: Record<string, FieldSchema>
  requiredFields: Set<string>
}

/** The free-text an "Other" selection carries per field, kept in the wizard so
 *  it survives step navigation (the per-step field remounts on Back/Next). */
type OtherText = Record<string, string>

// ─── One selectable option card ──────────────────────────────────────────────

function OptionCard({
  fieldName,
  option,
  control,
  selected,
  htmlFor,
  onSelect,
  labelId,
}: {
  fieldName: string
  option: RichOption
  control: React.ReactNode
  selected: boolean
  /** Radio path: forward the label click to the sibling control (idempotent). */
  htmlFor?: string
  /** Checkbox path: toggle directly (htmlFor would double-fire the toggle). */
  onSelect?: () => void
  /** Id of the visible label — the control references it via aria-labelledby so
   *  the accessible name includes the description/preview/recommended badge. */
  labelId: string
}) {
  const optId = `elicitation-field-${fieldName}-opt-${option.value}`
  return (
    <div
      className="flex items-start gap-3 rounded-lg border border-border p-3 transition-colors has-[[data-state=checked]]:border-primary has-[[data-state=checked]]:bg-accent/40"
      data-selected={selected || undefined}
    >
      <div className="mt-0.5 shrink-0">{control}</div>
      {/* Label holds ONLY phrasing content (spans) so it stays valid inside
          <label> — no <div>/<pre> nesting warnings. Radios use htmlFor (forwards
          the click to the sibling control, idempotent); checkboxes use onSelect
          (htmlFor + a sibling checkbox would double-fire the toggle). */}
      <label
        id={labelId}
        htmlFor={htmlFor}
        data-testid={optId}
        className="min-w-0 flex-1 cursor-pointer"
        onClick={onSelect}
      >
        <span className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium text-foreground">{option.label}</span>
          {option.recommended && (
            <Badge tone="primary" data-testid={`${optId}-recommended`}>
              Recommended
            </Badge>
          )}
        </span>
        {option.description && (
          <span className="mt-0.5 block text-xs text-muted-foreground">
            {option.description}
          </span>
        )}
        {option.preview && (
          <span
            data-testid={`${optId}-preview`}
            className="mt-1.5 block whitespace-pre-wrap rounded bg-muted px-2 py-1 font-mono text-xs text-foreground"
          >
            {option.preview}
          </span>
        )}
      </label>
    </div>
  )
}

// ─── The choice control for one question (single = radios, multi = checkboxes) ─

function ChoiceCards({
  name,
  fieldSchema,
  form,
  otherText,
  setOtherText,
}: {
  name: string
  fieldSchema: FieldSchema
  form: UseFormReturn<Record<string, unknown>>
  otherText: string
  setOtherText: (t: string) => void
}) {
  const uid = useId()
  const multi = isMultiChoiceField(fieldSchema)
  const options = useMemo(
    () => orderRecommendedFirst(getRichOptions(fieldSchema)),
    [fieldSchema],
  )
  const showOther = allowsOther(fieldSchema)
  // Derive the Other control's ids from the SENTINEL (not the literal "other"),
  // so they can only collide with an enum value equal to the reserved sentinel —
  // never with a realistic option value literally named "other".
  const otherId = `${uid}-${OTHER_SENTINEL}`
  const otherLabelId = `${uid}-${OTHER_SENTINEL}-label`
  // The question title/description name the option GROUP for a screen reader.
  const groupLabel = fieldSchema.title || name

  return (
    <Controller
      name={name}
      control={form.control}
      render={({ field, fieldState }) => {
        const otherOn = isOtherSelected(fieldSchema, field.value)

        const cards = options.map(opt => {
          const ctrlId = `${uid}-${opt.value}`
          const labelId = `${uid}-${opt.value}-label`
          if (multi) {
            const arr = Array.isArray(field.value) ? (field.value as string[]) : []
            const checked = arr.includes(opt.value)
            const toggle = () =>
              field.onChange(
                checked ? arr.filter(v => v !== opt.value) : [...arr, opt.value],
              )
            return (
              <OptionCard
                key={opt.value}
                fieldName={name}
                option={opt}
                selected={checked}
                onSelect={toggle}
                labelId={labelId}
                control={
                  <Checkbox
                    id={ctrlId}
                    checked={checked}
                    onCheckedChange={toggle}
                    onBlur={field.onBlur}
                    aria-labelledby={labelId}
                    data-testid={`${name}-${opt.value}-checkbox`}
                  />
                }
              />
            )
          }
          return (
            <OptionCard
              key={opt.value}
              fieldName={name}
              option={opt}
              selected={field.value === opt.value}
              htmlFor={ctrlId}
              labelId={labelId}
              control={
                <RadioGroupItem id={ctrlId} value={opt.value} aria-labelledby={labelId} />
              }
            />
          )
        })

        const toggleOtherMulti = () => {
          const arr = Array.isArray(field.value) ? (field.value as string[]) : []
          field.onChange(
            otherOn
              ? arr.filter(v => v !== OTHER_SENTINEL)
              : [...arr, OTHER_SENTINEL],
          )
        }
        const otherCard = showOther ? (
          <OptionCard
            key={OTHER_SENTINEL}
            fieldName={name}
            option={{ value: OTHER_SENTINEL, label: 'Other…' }}
            selected={otherOn}
            htmlFor={multi ? undefined : otherId}
            onSelect={multi ? toggleOtherMulti : undefined}
            labelId={otherLabelId}
            control={
              multi ? (
                <Checkbox
                  id={otherId}
                  checked={otherOn}
                  onCheckedChange={toggleOtherMulti}
                  aria-labelledby={otherLabelId}
                  data-testid={`${name}-${OTHER_SENTINEL}-checkbox`}
                />
              ) : (
                <RadioGroupItem
                  id={otherId}
                  value={OTHER_SENTINEL}
                  aria-labelledby={otherLabelId}
                />
              )
            }
          />
        ) : null

        return (
          <div className="flex flex-col gap-2" data-testid={`elicitation-field-${name}`}>
            {multi ? (
              <div role="group" aria-label={groupLabel} className="flex flex-col gap-2">
                {cards}
                {otherCard}
              </div>
            ) : (
              <RadioGroup
                value={typeof field.value === 'string' ? field.value : ''}
                onValueChange={field.onChange}
                aria-label={groupLabel}
              >
                {cards}
                {otherCard}
              </RadioGroup>
            )}

            {otherOn && (
              <Input
                value={otherText}
                onChange={e => setOtherText(e.target.value)}
                placeholder="Type your answer…"
                aria-label="Other value"
                data-testid={`elicitation-field-${name}-other-input`}
              />
            )}

            {fieldState.error?.message && (
              <span className="text-xs text-destructive" role="alert">
                {fieldState.error.message}
              </span>
            )}
          </div>
        )
      }}
    />
  )
}

/**
 * AskUserWizardContent — the rich `ask_user` decision UX.
 *
 * Renders each `properties` entry as one question. A single question shows no
 * wizard chrome; ≥2 questions become a Next/Back wizard with a single final
 * Submit. Choice questions render as selectable cards (radios / checkboxes) with
 * per-option descriptions, a recommended-first badge, an optional monospace
 * preview, and an always-available "Other" free-text escape. Non-choice
 * questions reuse the shared input controls. All answers submit as ONE flat
 * `{prop: value}` object — the same envelope the legacy form produces.
 */
export function AskUserWizardContent({
  elicitationId,
  message,
  server,
  properties,
  requiredFields,
}: AskUserWizardContentProps) {
  const entries = useMemo(() => Object.entries(properties), [properties])
  const total = entries.length

  const [step, setStep] = useState(0)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [otherText, setOtherTextState] = useState<OtherText>({})

  const formSchema = useMemo(
    () => buildFormSchema(properties, requiredFields),
    [properties, requiredFields],
  )
  const defaultValues = useMemo(
    () =>
      Object.fromEntries(
        entries.map(([key, fs]) => [
          key,
          fs.default ?? (isMultiChoiceField(fs) ? [] : undefined),
        ]),
      ),
    [entries],
  )
  const form = useForm<Record<string, unknown>>({
    resolver: zodResolver(formSchema),
    defaultValues,
  })

  // Empty `properties` (a malformed ask_user schema) → no field, just message +
  // actions; NEVER crash the render (the legacy path also tolerated this).
  const current = entries[step] ?? entries[0]
  const currentName = current?.[0] ?? ''
  const currentSchema = (current?.[1] ?? {}) as FieldSchema
  const isLast = step >= total - 1
  const isChoice = isSingleChoiceField(currentSchema) || isMultiChoiceField(currentSchema)
  const setOtherText = (name: string, t: string) =>
    setOtherTextState(prev => ({ ...prev, [name]: t }))

  // Move focus to the new question on step change (skip the initial mount) and
  // let the aria-live step indicator announce it — keyboard/SR users keep place.
  const questionRef = useRef<HTMLDivElement>(null)
  const mounted = useRef(false)
  useEffect(() => {
    if (!mounted.current) {
      mounted.current = true
      return
    }
    questionRef.current?.focus()
  }, [step])

  // Validate the current step (zod for the field + the Other-filled rule).
  const validateStep = async (name: string, fs: FieldSchema): Promise<boolean> => {
    const zodOk = await form.trigger(name)
    const otherErr = otherFieldError(fs, form.getValues(name), otherText[name])
    if (otherErr) {
      form.setError(name, { type: 'other-required', message: otherErr })
      return false
    }
    return zodOk
  }

  const handleNext = async () => {
    if (await validateStep(currentName, currentSchema)) setStep(s => Math.min(s + 1, total - 1))
  }
  const handleBack = () => setStep(s => Math.max(s - 1, 0))

  const handleDecline = async () => {
    if (isSubmitting) return
    setIsSubmitting(true)
    try {
      await Stores.McpComposer.resolveElicitation(elicitationId, 'decline')
    } catch (e) {
      // The store rolls status back on POST failure so the user can retry; swallow
      // so it doesn't bubble to the chat error boundary.
      console.warn('ask_user decline failed', e)
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleSubmit = async () => {
    if (isSubmitting) return
    // Set the guard SYNCHRONOUSLY before the first await so React flushes the
    // loading/disabled state before any second discrete click — otherwise the
    // `await form.trigger()` window lets a double-click (or a Submit-then-Decline)
    // re-enter with isSubmitting still false and issue a conflicting second POST.
    setIsSubmitting(true)
    try {
      const zodOk = await form.trigger()
      // Find the GLOBALLY-first offending step across BOTH zod + Other-filled rules.
      let firstBad = -1
      for (let i = 0; i < total; i++) {
        const [name, fs] = entries[i] as [string, FieldSchema]
        const otherErr = otherFieldError(fs, form.getValues(name), otherText[name])
        if (otherErr) form.setError(name, { type: 'other-required', message: otherErr })
        const invalid = otherErr != null || form.formState.errors[name] != null
        if (invalid && firstBad === -1) firstBad = i
      }
      if (!zodOk || firstBad >= 0) {
        if (firstBad >= 0) setStep(firstBad)
        return // `finally` re-enables the controls so the user can correct + retry
      }
      const values = finalizeValues(properties, form.getValues(), otherText)
      await Stores.McpComposer.resolveElicitation(elicitationId, 'accept', values)
    } catch (e) {
      console.warn('ask_user resolve failed', e)
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Card
      size="sm"
      className="mb-2"
      data-testid="mcp-elicitation-pending-card"
      footer={
        <div className="flex w-full items-center justify-between gap-2">
          <Button
            type="button"
            variant="ghost"
            onClick={handleDecline}
            loading={isSubmitting}
            size="default"
            data-testid="elicitation-decline"
          >
            Decline
          </Button>
          <div className="flex gap-2">
            {step > 0 && (
              <Button
                type="button"
                variant="outline"
                onClick={handleBack}
                disabled={isSubmitting}
                size="default"
                data-testid="elicitation-back"
              >
                Back
              </Button>
            )}
            {isLast ? (
              <Button
                type="button"
                loading={isSubmitting}
                size="default"
                onClick={handleSubmit}
                data-testid="elicitation-submit"
              >
                Submit
              </Button>
            ) : (
              <Button
                type="button"
                size="default"
                onClick={handleNext}
                disabled={isSubmitting}
                data-testid="elicitation-next"
              >
                Next
              </Button>
            )}
          </div>
        </div>
      }
    >
      <div className="flex items-center gap-2 min-w-0" data-testid="elicitation-wizard">
        <SquarePen className="size-4 shrink-0 text-primary" />
        <Text strong className="truncate">{server}</Text>
        <Text type="secondary" className="text-xs whitespace-nowrap">
          is requesting input
        </Text>
        {total > 1 && (
          <Text
            type="secondary"
            className="ms-auto text-xs whitespace-nowrap"
            aria-live="polite"
            data-testid="elicitation-wizard-step"
          >
            Step {step + 1} of {total}
          </Text>
        )}
      </div>

      <div className="mt-2">
        <Text className="text-sm">{message}</Text>
        <Form
          form={form}
          layout="vertical"
          className="mt-3"
          disabled={isSubmitting}
          data-testid="mcp-elicitation-form"
          onSubmit={() => (isLast ? handleSubmit() : handleNext())}
        >
          {/* tabIndex=-1 so step-change focus lands here (announced by aria-live). */}
          <div ref={questionRef} tabIndex={-1} className="outline-none">
            {current && isChoice && currentSchema.title && (
              <Text strong className="mb-1 block text-sm">
                {currentSchema.title}
              </Text>
            )}
            {current && isChoice && currentSchema.description && (
              <Text type="secondary" className="mb-2 block text-xs">
                {currentSchema.description}
              </Text>
            )}
            {current &&
              (isChoice ? (
                <ChoiceCards
                  name={currentName}
                  fieldSchema={currentSchema}
                  form={form}
                  otherText={otherText[currentName] ?? ''}
                  setOtherText={t => setOtherText(currentName, t)}
                />
              ) : (
                renderInputField(currentName, currentSchema, requiredFields.has(currentName))
              ))}
          </div>
        </Form>
      </div>
    </Card>
  )
}
