import { useId, useState } from 'react'
import {
  Badge,
  Button,
  Card,
  Controller,
  Form,
  Input,
  Text,
  useForm,
  zodResolver,
  type UseFormReturn,
} from '@/components/ui'
import { RadioGroup, RadioGroupItem } from '@/components/ui/shadcn/radio-group'
import { Checkbox } from '@/components/ui/shadcn/checkbox'
import { SquarePen } from 'lucide-react'
import { Stores } from '@/core/stores'
import {
  allowsOther,
  buildFormSchema,
  getRichOptions,
  isMultiChoiceField,
  isSingleChoiceField,
  orderRecommendedFirst,
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

// ─── Pure value helpers (Other-escape ⇄ response envelope) ───────────────────

/** True when a choice value currently sits on the "Other" free-text option. */
function isOtherSelected(fs: FieldSchema, value: unknown): boolean {
  if (isSingleChoiceField(fs)) return value === OTHER_SENTINEL
  if (isMultiChoiceField(fs))
    return Array.isArray(value) && value.includes(OTHER_SENTINEL)
  return false
}

/** Validation message when Other is selected but its free text is empty. */
function otherFieldError(
  fs: FieldSchema,
  value: unknown,
  otherText: string | undefined,
): string | null {
  if (isOtherSelected(fs, value) && !(otherText ?? '').trim())
    return 'Enter a value for “Other”.'
  return null
}

/** Replace the OTHER sentinel with the typed free text before submitting, so the
 *  response envelope never leaks the sentinel and the model gets the real value. */
function finalizeValues(
  properties: Record<string, FieldSchema>,
  values: Record<string, unknown>,
  otherText: OtherText,
): Record<string, unknown> {
  const out: Record<string, unknown> = { ...values }
  for (const [name, fs] of Object.entries(properties)) {
    const custom = (otherText[name] ?? '').trim()
    if (isSingleChoiceField(fs) && out[name] === OTHER_SENTINEL) {
      out[name] = custom
    } else if (
      isMultiChoiceField(fs) &&
      Array.isArray(out[name]) &&
      (out[name] as string[]).includes(OTHER_SENTINEL)
    ) {
      const kept = (out[name] as string[]).filter(v => v !== OTHER_SENTINEL)
      out[name] = custom ? [...kept, custom] : kept
    }
  }
  return out
}

// ─── One selectable option card ──────────────────────────────────────────────

function OptionCard({
  fieldName,
  option,
  control,
  selected,
  htmlFor,
  onSelect,
}: {
  fieldName: string
  option: RichOption
  control: React.ReactNode
  selected: boolean
  /** Radio path: forward the label click to the sibling control (idempotent). */
  htmlFor?: string
  /** Checkbox path: toggle directly (htmlFor would double-fire the toggle). */
  onSelect?: () => void
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
  const options = orderRecommendedFirst(getRichOptions(fieldSchema))
  const showOther = allowsOther(fieldSchema)
  const otherId = `${uid}-other`

  return (
    <Controller
      name={name}
      control={form.control}
      render={({ field, fieldState }) => {
        const otherOn = isOtherSelected(fieldSchema, field.value)

        const cards = options.map(opt => {
          const ctrlId = `${uid}-${opt.value}`
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
                control={
                  <Checkbox
                    id={ctrlId}
                    checked={checked}
                    onCheckedChange={toggle}
                    onBlur={field.onBlur}
                    aria-label={opt.label}
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
              control={
                <RadioGroupItem id={ctrlId} value={opt.value} aria-label={opt.label} />
              }
            />
          )
        })

        // The always-available Other escape (unless x-ziee-allow-other:false).
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
            control={
              multi ? (
                <Checkbox
                  id={otherId}
                  checked={otherOn}
                  onCheckedChange={toggleOtherMulti}
                  aria-label="Other"
                />
              ) : (
                <RadioGroupItem id={otherId} value={OTHER_SENTINEL} aria-label="Other" />
              )
            }
          />
        ) : null

        const list = (
          <div className="flex flex-col gap-2">
            {cards}
            {otherCard}
          </div>
        )

        return (
          <div className="flex flex-col gap-2" data-testid={`elicitation-field-${name}`}>
            {multi ? (
              list
            ) : (
              <RadioGroup
                value={typeof field.value === 'string' ? field.value : ''}
                onValueChange={field.onChange}
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
  const entries = Object.entries(properties)
  const total = entries.length

  const [step, setStep] = useState(0)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [otherText, setOtherTextState] = useState<OtherText>({})

  const formSchema = buildFormSchema(properties, requiredFields)
  const defaultValues = Object.fromEntries(
    entries.map(([key, fs]) => [
      key,
      fs.default ?? (isMultiChoiceField(fs) ? [] : undefined),
    ]),
  )
  const form = useForm<Record<string, unknown>>({
    resolver: zodResolver(formSchema),
    defaultValues,
  })

  const [currentName, currentSchema] = entries[step] ?? entries[0]
  const isLast = step >= total - 1
  const setOtherText = (name: string, t: string) =>
    setOtherTextState(prev => ({ ...prev, [name]: t }))

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
    if (await validateStep(currentName, currentSchema as FieldSchema))
      setStep(s => Math.min(s + 1, total - 1))
  }
  const handleBack = () => setStep(s => Math.max(s - 1, 0))

  const handleDecline = async () => {
    setIsSubmitting(true)
    try {
      await Stores.McpComposer.resolveElicitation(elicitationId, 'decline')
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleSubmit = async () => {
    // Validate EVERY field (zod) + every Other-filled rule; jump to the first
    // offending step so the user sees what's missing.
    const zodOk = await form.trigger()
    for (let i = 0; i < total; i++) {
      const [name, fs] = entries[i]
      const otherErr = otherFieldError(
        fs as FieldSchema,
        form.getValues(name),
        otherText[name],
      )
      if (otherErr) {
        form.setError(name, { type: 'other-required', message: otherErr })
        setStep(i)
        return
      }
    }
    if (!zodOk) {
      const firstBad = entries.findIndex(([name]) => form.formState.errors[name])
      if (firstBad >= 0) setStep(firstBad)
      return
    }
    setIsSubmitting(true)
    try {
      const values = finalizeValues(properties, form.getValues(), otherText)
      await Stores.McpComposer.resolveElicitation(elicitationId, 'accept', values)
    } catch (e) {
      // The store rolls status back to 'pending' on POST failure so the user can
      // retry; swallow so it doesn't bubble to the chat error boundary.
      console.warn('ask_user resolve failed', e)
    } finally {
      setIsSubmitting(false)
    }
  }

  const fs = currentSchema as FieldSchema
  const isChoice = isSingleChoiceField(fs) || isMultiChoiceField(fs)

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
            className="ml-auto text-xs whitespace-nowrap"
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
          {(currentSchema as FieldSchema).title && isChoice && (
            <Text strong className="mb-1 block text-sm">
              {(currentSchema as FieldSchema).title}
            </Text>
          )}
          {(currentSchema as FieldSchema).description && isChoice && (
            <Text type="secondary" className="mb-2 block text-xs">
              {(currentSchema as FieldSchema).description}
            </Text>
          )}
          {isChoice ? (
            <ChoiceCards
              name={currentName}
              fieldSchema={fs}
              form={form}
              otherText={otherText[currentName] ?? ''}
              setOtherText={t => setOtherText(currentName, t)}
            />
          ) : (
            renderInputField(currentName, fs, requiredFields.has(currentName))
          )}
        </Form>
      </div>
    </Card>
  )
}
