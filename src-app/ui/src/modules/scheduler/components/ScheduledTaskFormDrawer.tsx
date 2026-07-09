import { useEffect, useState } from 'react'

import type { CreateScheduledTask, TestFireResult } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import {
  Alert,
  Button,
  Flex,
  Input,
  Segmented,
  Spin,
  Switch,
  Text,
  Textarea,
  message,
} from '@/components/ui'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'

import { type ScheduleValue, ScheduleBuilder } from './ScheduleBuilder'

const browserTz = (): string => {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  } catch {
    return 'UTC'
  }
}

interface FormState {
  name: string
  target_kind: 'workflow' | 'prompt'
  workflow_id: string
  inputs_json: string
  assistant_id: string
  prompt: string
  model_id: string
  notify_mode: boolean // true = always (toast); false = silent
  notify_on_change: boolean // true = on_change; false = every run
  schedule: ScheduleValue
}

const blank = (): FormState => ({
  name: '',
  target_kind: 'prompt',
  workflow_id: '',
  inputs_json: '{}',
  assistant_id: '',
  prompt: '',
  model_id: '',
  notify_mode: true,
  notify_on_change: false,
  schedule: {
    schedule_kind: 'recurring',
    cron_expr: '0 9 * * 1',
    timezone: browserTz(),
  },
})

export function ScheduledTaskFormDrawer() {
  const { open, editing, loading } = Stores.SchedulerDrawer
  const canUse = usePermission(Permissions.SchedulerUse)
  const [f, setF] = useState<FormState>(blank())
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestFireResult | null>(null)

  useEffect(() => {
    if (!open) return
    if (editing) {
      setF({
        name: editing.name,
        target_kind: editing.target_kind === 'workflow' ? 'workflow' : 'prompt',
        workflow_id: editing.workflow_id ?? '',
        inputs_json: JSON.stringify(editing.inputs_json ?? {}, null, 2),
        assistant_id: editing.assistant_id ?? '',
        prompt: editing.prompt ?? '',
        model_id: editing.model_id ?? '',
        notify_mode: editing.notify_mode !== 'silent',
        notify_on_change: editing.notify_on === 'on_change',
        schedule: {
          schedule_kind:
            editing.schedule_kind === 'once' ? 'once' : 'recurring',
          run_at: editing.run_at ?? undefined,
          cron_expr: editing.cron_expr ?? undefined,
          timezone: editing.timezone,
        },
      })
    } else {
      setF(blank())
    }
    setTestResult(null)
  }, [open, editing])

  const buildBody = (): CreateScheduledTask => {
    let inputs: unknown = {}
    try {
      inputs = JSON.parse(f.inputs_json || '{}')
    } catch {
      /* validated on submit */
    }
    return {
      name: f.name.trim(),
      target_kind: f.target_kind,
      workflow_id:
        f.target_kind === 'workflow' ? f.workflow_id || undefined : undefined,
      inputs_json: inputs as CreateScheduledTask['inputs_json'],
      assistant_id:
        f.target_kind === 'prompt' ? f.assistant_id || undefined : undefined,
      prompt: f.target_kind === 'prompt' ? f.prompt : undefined,
      model_id: f.model_id,
      schedule_kind: f.schedule.schedule_kind,
      run_at: f.schedule.run_at,
      cron_expr: f.schedule.cron_expr,
      timezone: f.schedule.timezone,
      notify_mode: f.notify_mode ? 'always' : 'silent',
      notify_on: f.notify_on_change ? 'on_change' : 'always',
    }
  }

  const validate = (): string | null => {
    if (!f.name.trim()) return 'Name is required'
    if (!f.model_id.trim()) return 'A model is required'
    if (f.target_kind === 'workflow' && !f.workflow_id.trim())
      return 'Workflow is required'
    if (f.target_kind === 'prompt' && !f.prompt.trim())
      return 'Prompt is required'
    if (f.schedule.schedule_kind === 'once' && !f.schedule.run_at)
      return 'A run date/time is required'
    if (f.schedule.schedule_kind === 'recurring' && !f.schedule.cron_expr?.trim())
      return 'A schedule is required'
    if (!f.schedule.timezone.trim()) return 'A timezone is required'
    if (f.target_kind === 'workflow') {
      try {
        JSON.parse(f.inputs_json || '{}')
      } catch {
        return 'Inputs must be valid JSON'
      }
    }
    return null
  }

  const handleSave = async () => {
    const err = validate()
    if (err) {
      message.error(err)
      return
    }
    Stores.SchedulerDrawer.setLoading(true)
    try {
      if (editing) {
        await Stores.ScheduledTasks.updateTask(editing.id, buildBody())
        message.success('Task updated')
      } else {
        await Stores.ScheduledTasks.createTask(buildBody())
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
    const err = validate()
    if (err) {
      message.error(err)
      return
    }
    setTesting(true)
    setTestResult(null)
    try {
      const result = await Stores.ScheduledTasks.testFire({
        target_kind: f.target_kind,
        workflow_id:
          f.target_kind === 'workflow' ? f.workflow_id || undefined : undefined,
        inputs_json: JSON.parse(f.inputs_json || '{}'),
        assistant_id:
          f.target_kind === 'prompt' ? f.assistant_id || undefined : undefined,
        prompt: f.target_kind === 'prompt' ? f.prompt : undefined,
        model_id: f.model_id,
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
            onClick={handleTest}
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
              onClick={handleSave}
              loading={loading}
            >
              {editing ? 'Save' : 'Create'}
            </Button>
          )}
        </Flex>
      }
    >
      <Flex className="flex-col gap-4">
        <label className="flex flex-col gap-1">
          <Text className="text-sm">Name</Text>
          <Input
            data-testid="task-form-name"
            value={f.name}
            onChange={e => setF({ ...f, name: e.target.value })}
            placeholder="Weekly CRISPR papers"
            autoFocus
          />
        </label>

        <Segmented
          data-testid="task-form-target-kind"
          value={f.target_kind}
          onChange={v =>
            setF({ ...f, target_kind: v as 'workflow' | 'prompt' })
          }
          options={[
            { label: 'Prompt', value: 'prompt' },
            { label: 'Workflow', value: 'workflow' },
          ]}
        />

        {f.target_kind === 'prompt' ? (
          <>
            <label className="flex flex-col gap-1">
              <Text className="text-sm">Prompt</Text>
              <Textarea
                data-testid="task-form-prompt"
                rows={4}
                value={f.prompt}
                onChange={e => setF({ ...f, prompt: e.target.value })}
                placeholder="Search PubMed and arXiv for new papers on… and summarize."
              />
            </label>
            <label className="flex flex-col gap-1">
              <Text className="text-sm">Assistant ID (optional)</Text>
              <Input
                data-testid="task-form-assistant"
                value={f.assistant_id}
                onChange={e => setF({ ...f, assistant_id: e.target.value })}
                placeholder="defaults to your default assistant"
              />
            </label>
          </>
        ) : (
          <>
            <label className="flex flex-col gap-1">
              <Text className="text-sm">Workflow ID</Text>
              <Input
                data-testid="task-form-workflow"
                value={f.workflow_id}
                onChange={e => setF({ ...f, workflow_id: e.target.value })}
              />
            </label>
            <label className="flex flex-col gap-1">
              <Text className="text-sm">Inputs (JSON)</Text>
              <Textarea
                data-testid="task-form-inputs"
                rows={3}
                value={f.inputs_json}
                onChange={e => setF({ ...f, inputs_json: e.target.value })}
              />
            </label>
          </>
        )}

        <label className="flex flex-col gap-1">
          <Text className="text-sm">Model ID</Text>
          <Input
            data-testid="task-form-model"
            value={f.model_id}
            onChange={e => setF({ ...f, model_id: e.target.value })}
          />
        </label>

        <div>
          <Text className="mb-1 text-sm">Schedule</Text>
          <ScheduleBuilder
            value={f.schedule}
            onChange={schedule => setF({ ...f, schedule })}
          />
        </div>

        <Flex className="items-center justify-between">
          <Text className="text-sm">Show a toast when it runs</Text>
          <Switch
            data-testid="task-form-notify-mode"
            checked={f.notify_mode}
            onCheckedChange={v => setF({ ...f, notify_mode: v })}
          />
        </Flex>
        <Flex className="items-center justify-between">
          <Text className="text-sm">Only notify when results change</Text>
          <Switch
            data-testid="task-form-notify-on-change"
            checked={f.notify_on_change}
            onCheckedChange={v => setF({ ...f, notify_on_change: v })}
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
      </Flex>
    </Drawer>
  )
}
