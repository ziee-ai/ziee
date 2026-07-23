import { useEffect, useState } from 'react'
import { z } from 'zod'

import {
  Button,
  Flex,
  Form,
  FormField,
  Input,
  message,
  Segmented,
  Textarea,
  useForm,
  zodResolver,
} from '@ziee/kit'
import { Field, FieldDescription, FieldError, FieldTitle } from '@ziee/kit/shadcn/field'

import type { CreateScheduledTask } from '@/api-client/types'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
// Reuse the scheduler module's proven schedule primitives (PLAN ITEM-20 — "mirror
// ScheduleBuilder.tsx"). ScheduleBuilder emits a 5-field POSIX cron / datetime and
// shows the auto-detected timezone read-only; ModelField is the grouped model Select.
import {
  ScheduleBuilder,
  type ScheduleValue,
} from '@/modules/scheduler/components/ScheduleBuilder'
import { ModelField } from '@/modules/scheduler/components/TaskTargetPickers'
import { ModelPicker } from '@/modules/user-llm-providers/modelPicker'
import { ScheduledTasks } from '@/modules/scheduler/stores/scheduledTasks'

/** Auto-detected IANA timezone (never an input the user fills — input-economy). */
const browserTz = (): string => {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  } catch {
    return 'UTC'
  }
}

// Keep in lock-step with the backend `MAX_COMPLETION_CONDITION_LEN` (scheduler
// models.rs) so an over-long condition fails inline instead of as a 400.
const MAX_COMPLETION_CONDITION_LEN = 4096

/**
 * `schedule` = a fixed Once/Recurring task (backend `schedule_kind` once|recurring);
 * `loop` = a self-paced run (`schedule_kind = 'self_paced'`) that decides its own
 * cadence and, when a completion condition is given, becomes a goal-seeking loop.
 */
type Mode = 'schedule' | 'loop'

interface FormValues {
  mode: Mode
  /** Optional — falls back to the message text when blank. */
  name: string
  prompt: string
  model_id: string
  /** Once/Recurring builder value (used only in `schedule` mode). */
  schedule: ScheduleValue
  /** Goal-seeking "done when…" (used only in `loop` mode; ITEM-24). */
  completion_condition: string
}

const formSchema = z
  .object({
    mode: z.enum(['schedule', 'loop']),
    name: z.string(),
    prompt: z.string().trim().min(1, 'A message is required'),
    model_id: z.string().trim().min(1, 'A model is required'),
    schedule: z.object({
      schedule_kind: z.enum(['once', 'recurring']),
      run_at: z.string().optional(),
      cron_expr: z.string().optional(),
      timezone: z.string().min(1, 'A timezone is required'),
    }),
    completion_condition: z
      .string()
      .max(MAX_COMPLETION_CONDITION_LEN, 'The completion condition is too long'),
  })
  .superRefine((v, ctx) => {
    // The schedule fields only apply in `schedule` mode; a self-paced loop needs
    // neither a run-at nor a cron (the backend `validate_schedule` relaxes both).
    if (v.mode !== 'schedule') return
    if (v.schedule.schedule_kind === 'once' && !v.schedule.run_at)
      ctx.addIssue({
        code: 'custom',
        path: ['schedule'],
        message: 'A run date and time is required',
      })
    if (v.schedule.schedule_kind === 'recurring' && !v.schedule.cron_expr?.trim())
      ctx.addIssue({
        code: 'custom',
        path: ['schedule'],
        message: 'A schedule is required',
      })
  })

interface Props {
  open: boolean
  onClose: () => void
  /** The conversation this schedule/loop binds to (DEC-46 — its firings land here). */
  conversationId: string
  /** Seed the model with the conversation's current model when available. */
  defaultModelId?: string
}

/**
 * In-chat "Schedule or loop this chat" dialog (Group E, ITEM-18/20). A single
 * merged form with two modes:
 *   - Schedule (Once / Recurring) → a `scheduled_task` bound to this conversation.
 *   - Loop (self-paced) → a `self_paced` task that paces itself, optionally
 *     goal-seeking via a natural-language completion condition (ITEM-24 FE).
 *
 * Every firing appends to THIS conversation (`bound_conversation_id`). Reuses the
 * scheduler module's `ScheduledTasks` store + `ScheduleBuilder`/`ModelField` — no
 * new store, no forked schedule logic. Rendered as a `Drawer` (the same overlay
 * the standalone `ScheduledTaskFormDrawer` uses) so its Select/datetime controls
 * behave identically to the proven sibling.
 */
export function ScheduleLoopDialog({
  open,
  onClose,
  conversationId,
  defaultModelId,
}: Props) {
  const form = useForm<FormValues>({
    resolver: zodResolver(formSchema),
    defaultValues: {
      mode: 'schedule',
      name: '',
      prompt: '',
      model_id: '',
      schedule: {
        schedule_kind: 'recurring',
        cron_expr: '0 9 * * 1',
        timezone: browserTz(),
      },
      completion_condition: '',
    },
  })
  const [saving, setSaving] = useState(false)

  const mode = form.watch('mode')
  const schedule = form.watch('schedule')

  // On open: populate the model list (self-gated) and reset to a clean form,
  // seeding the model from the conversation's own model (else the user's default).
  useEffect(() => {
    if (!open) return
    void (async () => {
      await ModelPicker.loadProviders()
      // defaultModelId() is a lazy store action → it returns a Promise; it must
      // be awaited (using the Promise directly would set model_id to a Promise).
      const model_id =
        defaultModelId || (await ModelPicker.defaultModelId()) || ''
      form.reset({
        mode: 'schedule',
        name: '',
        prompt: '',
        model_id,
        schedule: {
          schedule_kind: 'recurring',
          cron_expr: '0 9 * * 1',
          timezone: browserTz(),
        },
        completion_condition: '',
      })
    })()
  }, [open, defaultModelId, form])

  const buildBody = (v: FormValues): CreateScheduledTask => {
    const isLoop = v.mode === 'loop'
    const condition = v.completion_condition.trim()
    // Name is optional in-chat (input-economy) — derive from the message when blank
    // (the backend requires a non-empty name ≤ 255 chars).
    const name = (v.name.trim() || v.prompt.trim().slice(0, 80)).slice(0, 255)
    return {
      name,
      target_kind: 'prompt',
      prompt: v.prompt.trim(),
      model_id: v.model_id,
      schedule_kind: isLoop ? 'self_paced' : v.schedule.schedule_kind,
      run_at:
        !isLoop && v.schedule.schedule_kind === 'once'
          ? v.schedule.run_at
          : undefined,
      cron_expr:
        !isLoop && v.schedule.schedule_kind === 'recurring'
          ? v.schedule.cron_expr
          : undefined,
      timezone: v.schedule.timezone,
      bound_conversation_id: conversationId,
      // Goal-seeking only applies to a self-paced loop (backend rejects it
      // otherwise); a blank condition ⇒ an ordinary self-paced loop.
      completion_condition: isLoop && condition ? condition : undefined,
    }
  }

  const onSubmit = async (v: FormValues) => {
    setSaving(true)
    try {
      await ScheduledTasks.createTask(buildBody(v))
      message.success(v.mode === 'loop' ? 'Loop started' : 'Task scheduled')
      onClose()
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to save')
    } finally {
      setSaving(false)
    }
  }

  return (
    <Drawer
      title="Schedule or loop this chat"
      open={open}
      onClose={onClose}
      size={520}
      destroyOnHidden
      footer={
        <Flex className="justify-end gap-2">
          <Button
            data-testid="schedule-loop-cancel"
            variant="outline"
            onClick={onClose}
            disabled={saving}
          >
            Cancel
          </Button>
          <Button
            data-testid="schedule-loop-submit"
            loading={saving}
            onClick={form.handleSubmit(onSubmit, errors => {
              // Surface the first validation error for controls without an inline
              // FieldError (e.g. the Schedule block).
              for (const err of Object.values(errors)) {
                const m = (err as { message?: string } | undefined)?.message
                if (m) {
                  message.error(m)
                  break
                }
              }
            })}
          >
            {mode === 'loop' ? 'Start loop' : 'Schedule'}
          </Button>
        </Flex>
      }
    >
      <Form
        data-testid="schedule-loop-form"
        form={form}
        layout="vertical"
        onSubmit={onSubmit}
      >
        <FormField name="mode" label="Mode">
          {/* aria-label is explicit: Segmented forwards aria-label but not the
              aria-labelledby FormField injects (mirror ScheduledTaskFormDrawer). */}
          <Segmented
            data-testid="schedule-loop-mode"
            aria-label="Mode"
            options={[
              { label: 'Schedule', value: 'schedule' },
              { label: 'Loop', value: 'loop' },
            ]}
          />
        </FormField>

        <FormField name="prompt" label="Message" required>
          <Textarea
            data-testid="schedule-loop-prompt"
            rows={4}
            autoFocus
            placeholder={
              mode === 'loop'
                ? 'Keep checking the sequencing run and summarise progress…'
                : 'Search PubMed for new CRISPR papers and summarise them…'
            }
          />
        </FormField>

        <FormField
          name="name"
          label="Name"
          description="Optional — defaults to your message."
        >
          <Input
            data-testid="schedule-loop-name"
            placeholder="Auto-named from your message"
          />
        </FormField>

        <FormField name="model_id" label="Model" required>
          <ModelField />
        </FormField>

        {mode === 'schedule' ? (
          // Schedule is a compound control with required value/onChange props, so it
          // can't be cloned by FormField (which injects those) — wrap in a labelled
          // Field. zod validates it; the footer's onInvalid surfaces a missing
          // run-at/cron (mirror ScheduledTaskFormDrawer).
          <Field>
            <FieldTitle>Schedule</FieldTitle>
            <ScheduleBuilder
              value={schedule}
              onChange={next =>
                form.setValue('schedule', next, {
                  shouldValidate: form.formState.isSubmitted,
                })
              }
            />
            {form.formState.errors.schedule?.message && (
              <FieldError data-testid="schedule-loop-schedule-error">
                {String(form.formState.errors.schedule.message)}
              </FieldError>
            )}
          </Field>
        ) : (
          <>
            <FormField
              name="completion_condition"
              label="Stop when…"
              description="Describe how the loop knows it's done. Leave blank to loop until the maximum turns or time limit set by your administrator."
            >
              <Textarea
                data-testid="schedule-loop-completion"
                rows={3}
                placeholder="the QC figure passes and there are no missing values"
              />
            </FormField>
            <Field>
              <FieldDescription data-testid="schedule-loop-selfpaced-note">
                The assistant chooses when to run next and keeps going until the
                condition above is met — or your administrator's maximum turns and
                time horizon are reached.
              </FieldDescription>
            </Field>
          </>
        )}
      </Form>
    </Drawer>
  )
}
