import { useEffect, useMemo, useState } from 'react'

import type { CreateScheduledTask, TestFireResult } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import {
  Alert,
  Button,
  Flex,
  Form,
  FormField,
  Input,
  Segmented,
  Spin,
  Switch,
  Text,
  Textarea,
  useForm,
  message,
} from '@/components/ui'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'

import { chooseInputMode, selectDeclaredInputs } from './inputMode'
import { type ScheduleValue, ScheduleBuilder } from './ScheduleBuilder'
import {
  type AllowedUnattendedTool,
  AllowedToolsField,
  AssistantField,
  ModelField,
  WorkflowField,
} from './TaskTargetPickers'

const browserTz = (): string => {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  } catch {
    return 'UTC'
  }
}

interface FormValues {
  name: string
  target_kind: 'workflow' | 'prompt'
  workflow_id: string
  /** Free-form JSON inputs — fallback ONLY when the workflow declares no inputs. */
  inputs_json: string
  /** Typed workflow inputs, keyed by input name (ITEM-4). */
  inputs: Record<string, string>
  assistant_id: string
  prompt: string
  model_id: string
  notify_mode: boolean // true = always (toast); false = silent
  notify_on_change: boolean // true = on_change; false = every run
  /** Tools the task may invoke unattended (ITEM-16 / DEC-17.4). Empty = safe. */
  allowed_unattended_tools: AllowedUnattendedTool[]
  schedule: ScheduleValue
}

const blank = (): FormValues => ({
  name: '',
  target_kind: 'prompt',
  workflow_id: '',
  inputs_json: '{}',
  inputs: {},
  assistant_id: '',
  prompt: '',
  model_id: '',
  notify_mode: true,
  notify_on_change: false,
  allowed_unattended_tools: [],
  schedule: {
    schedule_kind: 'recurring',
    cron_expr: '0 9 * * 1',
    timezone: browserTz(),
  },
})

export function ScheduledTaskFormDrawer() {
  const { open, editing, loading } = Stores.SchedulerDrawer
  const { workflows } = Stores.Workflow
  const canUse = usePermission(Permissions.SchedulerUse)
  const form = useForm<FormValues>({ defaultValues: blank() })
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestFireResult | null>(null)

  // Watched fields drive conditional rendering (the rest bind via FormField).
  const targetKind = form.watch('target_kind')
  const workflowId = form.watch('workflow_id')
  const notifyMode = form.watch('notify_mode')
  const notifyOnChange = form.watch('notify_on_change')
  const schedule = form.watch('schedule')

  // The selected workflow's declared inputs (ITEM-4). When it declares inputs we
  // render a typed field per input; otherwise fall back to a raw JSON textarea.
  const declaredInputs = useMemo(
    () => selectDeclaredInputs(workflows, workflowId),
    [workflows, workflowId],
  )
  const hasDeclaredInputs = chooseInputMode(declaredInputs) === 'typed'

  // Populate the picker lists on open (each store self-gates + loads once).
  useEffect(() => {
    if (!open) return
    void Stores.AssistantPicker.loadAssistants()
    void Stores.Workflow.loadWorkflows()
    void Stores.ModelPicker.loadProviders()
    void Stores.McpServer.loadMcpServers()
  }, [open])

  useEffect(() => {
    if (!open) return
    if (editing) {
      const inputsObj =
        editing.inputs_json && typeof editing.inputs_json === 'object'
          ? (editing.inputs_json as Record<string, unknown>)
          : {}
      form.reset({
        name: editing.name,
        target_kind: editing.target_kind === 'workflow' ? 'workflow' : 'prompt',
        workflow_id: editing.workflow_id ?? '',
        inputs_json: JSON.stringify(editing.inputs_json ?? {}, null, 2),
        inputs: Object.fromEntries(
          Object.entries(inputsObj).map(([k, v]) => [
            k,
            v == null ? '' : String(v),
          ]),
        ),
        assistant_id: editing.assistant_id ?? '',
        prompt: editing.prompt ?? '',
        model_id: editing.model_id ?? '',
        notify_mode: editing.notify_mode !== 'silent',
        notify_on_change: editing.notify_on === 'on_change',
        // The ScheduledTask response types this JSONB column loosely (`{}`);
        // it is always an AllowedUnattendedTool[] at runtime (mirror of the
        // create/update field). Cast to the form's typed shape.
        allowed_unattended_tools:
          (editing.allowed_unattended_tools as
            | AllowedUnattendedTool[]
            | null
            | undefined) ?? [],
        schedule: {
          schedule_kind:
            editing.schedule_kind === 'once' ? 'once' : 'recurring',
          run_at: editing.run_at ?? undefined,
          cron_expr: editing.cron_expr ?? undefined,
          timezone: editing.timezone,
        },
      })
    } else {
      form.reset(blank())
    }
    setTestResult(null)
  }, [open, editing, form])

  // Seed a '' default for any declared input the form doesn't have yet, so the
  // typed inputs render as controlled fields (mirrors WorkflowRunDialog).
  useEffect(() => {
    if (targetKind !== 'workflow' || !hasDeclaredInputs) return
    const current = form.getValues('inputs') || {}
    const seeded = { ...current }
    let changed = false
    for (const i of declaredInputs) {
      if (seeded[i.name] === undefined) {
        seeded[i.name] = i.default != null ? String(i.default) : ''
        changed = true
      }
    }
    if (changed) form.setValue('inputs', seeded)
  }, [declaredInputs, hasDeclaredInputs, targetKind, form])

  const buildInputs = (values: FormValues): unknown => {
    if (values.target_kind !== 'workflow') return {}
    if (hasDeclaredInputs) {
      return Object.fromEntries(
        declaredInputs.map(i => [i.name, values.inputs?.[i.name] ?? '']),
      )
    }
    try {
      return JSON.parse(values.inputs_json || '{}')
    } catch {
      return {}
    }
  }

  const buildBody = (values: FormValues): CreateScheduledTask => ({
    name: values.name.trim(),
    target_kind: values.target_kind,
    workflow_id:
      values.target_kind === 'workflow'
        ? values.workflow_id || undefined
        : undefined,
    inputs_json: buildInputs(values) as CreateScheduledTask['inputs_json'],
    assistant_id:
      values.target_kind === 'prompt'
        ? values.assistant_id || undefined
        : undefined,
    prompt: values.target_kind === 'prompt' ? values.prompt : undefined,
    model_id: values.model_id,
    schedule_kind: values.schedule.schedule_kind,
    run_at: values.schedule.run_at,
    cron_expr: values.schedule.cron_expr,
    timezone: values.schedule.timezone,
    notify_mode: values.notify_mode ? 'always' : 'silent',
    notify_on: values.notify_on_change ? 'on_change' : 'always',
    // ITEM-16: added to CreateScheduledTask by a separate OpenAPI regen.
    allowed_unattended_tools: values.allowed_unattended_tools,
  })

  const validate = (values: FormValues): string | null => {
    if (!values.name.trim()) return 'Name is required'
    if (!values.model_id.trim()) return 'A model is required'
    if (values.target_kind === 'workflow' && !values.workflow_id.trim())
      return 'Workflow is required'
    if (values.target_kind === 'prompt' && !values.prompt.trim())
      return 'Prompt is required'
    if (values.schedule.schedule_kind === 'once' && !values.schedule.run_at)
      return 'A run date/time is required'
    if (
      values.schedule.schedule_kind === 'recurring' &&
      !values.schedule.cron_expr?.trim()
    )
      return 'A schedule is required'
    if (!values.schedule.timezone.trim()) return 'A timezone is required'
    if (values.target_kind === 'workflow') {
      if (hasDeclaredInputs) {
        // Typed-input mode: every REQUIRED declared input must be non-empty
        // (blind-audit fix: FormField `required` is only decorative here — there's
        // no RHF resolver — so the rule must be enforced explicitly).
        for (const i of declaredInputs) {
          if (i.required && !String(values.inputs?.[i.name] ?? '').trim()) {
            return `${i.name} is required`
          }
        }
      } else {
        try {
          JSON.parse(values.inputs_json || '{}')
        } catch {
          return 'Inputs must be valid JSON'
        }
      }
    }
    return null
  }

  const handleSave = async () => {
    const values = form.getValues()
    const err = validate(values)
    if (err) {
      message.error(err)
      return
    }
    Stores.SchedulerDrawer.setLoading(true)
    try {
      if (editing) {
        await Stores.ScheduledTasks.updateTask(editing.id, buildBody(values))
        message.success('Task updated')
      } else {
        await Stores.ScheduledTasks.createTask(buildBody(values))
        message.success('Task created')
      }
      Stores.SchedulerDrawer.close()
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to save task')
    } finally {
      Stores.SchedulerDrawer.setLoading(false)
    }
  }

  const handleTest = async () => {
    const values = form.getValues()
    const err = validate(values)
    if (err) {
      message.error(err)
      return
    }
    setTesting(true)
    setTestResult(null)
    try {
      const result = await Stores.ScheduledTasks.testFire({
        target_kind: values.target_kind,
        workflow_id:
          values.target_kind === 'workflow'
            ? values.workflow_id || undefined
            : undefined,
        inputs_json: buildInputs(values),
        assistant_id:
          values.target_kind === 'prompt'
            ? values.assistant_id || undefined
            : undefined,
        prompt: values.target_kind === 'prompt' ? values.prompt : undefined,
        model_id: values.model_id,
      })
      setTestResult(result)
    } catch (e) {
      setTestResult({
        ok: false,
        text: '',
        error: e instanceof Error ? e.message : 'Test failed',
      })
    } finally {
      setTesting(false)
    }
  }

  return (
    <Drawer
      title={editing ? 'Edit scheduled task' : 'New scheduled task'}
      open={open}
      onClose={() => Stores.SchedulerDrawer.close()}
      size={640}
      destroyOnHidden
      footer={
        <Flex className="justify-end gap-2">
          <Button
            data-testid="task-form-test"
            variant="outline"
            onClick={() => void handleTest()}
            loading={testing}
            disabled={!canUse || loading}
          >
            Test
          </Button>
          <Button
            data-testid="task-form-cancel"
            variant="outline"
            onClick={() => Stores.SchedulerDrawer.close()}
            disabled={loading}
          >
            {canUse ? 'Cancel' : 'Close'}
          </Button>
          {canUse && (
            <Button
              data-testid="task-form-save"
              onClick={() => void handleSave()}
              loading={loading}
            >
              {editing ? 'Save' : 'Create'}
            </Button>
          )}
        </Flex>
      }
    >
      <Form
        data-testid="task-form"
        form={form}
        layout="vertical"
        disabled={!canUse}
        onSubmit={() => void handleSave()}
      >
        <FormField name="name" label="Name" required>
          <Input
            data-testid="task-form-name"
            placeholder="Weekly CRISPR papers"
            autoFocus
          />
        </FormField>

        <Segmented
          data-testid="task-form-target-kind"
          value={targetKind}
          onChange={v =>
            form.setValue('target_kind', v as 'workflow' | 'prompt')
          }
          options={[
            { label: 'Prompt', value: 'prompt' },
            { label: 'Workflow', value: 'workflow' },
          ]}
        />

        {targetKind === 'prompt' ? (
          <>
            <FormField name="prompt" label="Prompt" required>
              <Textarea
                data-testid="task-form-prompt"
                rows={4}
                placeholder="Search PubMed and arXiv for new papers on… and summarize."
              />
            </FormField>
            <FormField
              name="assistant_id"
              label="Assistant"
              description="Defaults to your default assistant."
            >
              <AssistantField />
            </FormField>
          </>
        ) : (
          <>
            <FormField name="workflow_id" label="Workflow" required>
              <WorkflowField />
            </FormField>
            {hasDeclaredInputs ? (
              declaredInputs.map(input => (
                <FormField
                  key={input.name}
                  name={`inputs.${input.name}`}
                  label={input.name}
                  description={input.description}
                  required={input.required}
                >
                  <Input
                    data-testid={`task-form-input-${input.name}`}
                    placeholder={input.description}
                  />
                </FormField>
              ))
            ) : (
              <FormField
                name="inputs_json"
                label="Inputs (JSON)"
                description="Provide inputs as a JSON object."
              >
                <Textarea data-testid="task-form-inputs" rows={3} />
              </FormField>
            )}
          </>
        )}

        <FormField name="model_id" label="Model" required>
          <ModelField />
        </FormField>

        <FormField
          name="allowed_unattended_tools"
          label="Tools this task may use unattended"
          description="Empty = only built-in read-only tools run unattended."
        >
          <AllowedToolsField />
        </FormField>

        <div>
          <Text className="mb-1 text-sm">Schedule</Text>
          <ScheduleBuilder
            value={schedule}
            onChange={next => form.setValue('schedule', next)}
          />
        </div>

        <Flex className="items-center justify-between">
          <Text className="text-sm">Show a toast when it runs</Text>
          <Switch
            data-testid="task-form-notify-mode"
            aria-label="Show a toast when the task runs"
            checked={notifyMode}
            onCheckedChange={v => form.setValue('notify_mode', v)}
          />
        </Flex>
        <Flex className="items-center justify-between">
          <Text className="text-sm">Only notify when results change</Text>
          <Switch
            data-testid="task-form-notify-on-change"
            aria-label="Only notify when results change"
            checked={notifyOnChange}
            onCheckedChange={v => form.setValue('notify_on_change', v)}
          />
        </Flex>

        {testing && (
          <Flex className="justify-center py-4">
            <Spin label="Running test…" />
          </Flex>
        )}
        {testResult && (
          <Alert
            data-testid="task-form-test-result"
            tone={testResult.ok ? 'success' : 'error'}
            title={testResult.ok ? 'Test result' : 'Test failed'}
            description={
              testResult.ok
                ? testResult.text || '(empty result)'
                : testResult.error
            }
          />
        )}
      </Form>
    </Drawer>
  )
}
