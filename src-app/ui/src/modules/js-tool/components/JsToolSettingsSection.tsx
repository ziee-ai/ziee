import { useEffect, useState } from 'react'
import {
  Alert,
  Card,
  Separator,
  Form,
  FormField,
  InputNumber,
  Spin,
  Text,
  Paragraph,
  useForm,
  zodResolver,
  message,
} from '@ziee/kit'
import { z } from 'zod'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { type JsToolSettings as JsToolSettingsRow, type UpdateJsToolSettings } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { JsToolSettings as JsToolSettingsStore } from '@/modules/js-tool/stores/jsToolSettings'

const MANAGE_PERM = Permissions.JsToolSettingsManage
const READ_PERM = Permissions.JsToolSettingsRead

const MIB = 1024 * 1024
const KIB = 1024

/** Form values mirror the API row but edit the byte caps in MiB / KiB. */
type FormValues = {
  memory_mib: number
  max_stack_kib: number
  wall_secs: number
  approval_timeout_secs: number
  max_concurrent_runs: number
  max_concurrent_dispatch: number
  max_trace_entries: number
}

const schema = z.object({
  memory_mib: z
    .number()
    .refine(v => v >= 16 && v <= 4096, 'must be 16 MiB ..= 4 GiB'),
  max_stack_kib: z
    .number()
    .refine(v => v >= 64 && v <= 65_536, 'must be 64 KiB ..= 64 MiB'),
  wall_secs: z.number().refine(v => v >= 1 && v <= 3600, 'must be 1..=3600'),
  approval_timeout_secs: z
    .number()
    .refine(v => v >= 5 && v <= 3600, 'must be 5..=3600'),
  max_concurrent_runs: z
    .number()
    .refine(v => v >= 1 && v <= 256, 'must be 1..=256'),
  max_concurrent_dispatch: z
    .number()
    .refine(v => v >= 1 && v <= 64, 'must be 1..=64'),
  max_trace_entries: z
    .number()
    .refine(v => v >= 1 && v <= 10_000, 'must be 1..=10000'),
})

const EMPTY_DEFAULTS: FormValues = {
  memory_mib: 128,
  max_stack_kib: 512,
  wall_secs: 300,
  approval_timeout_secs: 300,
  max_concurrent_runs: 8,
  max_concurrent_dispatch: 6,
  max_trace_entries: 256,
}

function rowToForm(row: JsToolSettingsRow): FormValues {
  return {
    memory_mib: Math.round(row.memory_bytes / MIB),
    max_stack_kib: Math.round(row.max_stack_bytes / KIB),
    wall_secs: row.wall_secs,
    approval_timeout_secs: row.approval_timeout_secs,
    max_concurrent_runs: row.max_concurrent_runs,
    max_concurrent_dispatch: row.max_concurrent_dispatch,
    max_trace_entries: row.max_trace_entries,
  }
}

function formToPatch(v: FormValues): UpdateJsToolSettings {
  return {
    memory_bytes: v.memory_mib * MIB,
    max_stack_bytes: v.max_stack_kib * KIB,
    wall_secs: v.wall_secs,
    approval_timeout_secs: v.approval_timeout_secs,
    max_concurrent_runs: v.max_concurrent_runs,
    max_concurrent_dispatch: v.max_concurrent_dispatch,
    max_trace_entries: v.max_trace_entries,
  }
}

/**
 * Admin section for the run_js (js_tool) resource caps — a singleton-row Form +
 * Save/Reset. Permission-aware: without `js_tool::settings::manage` the form is
 * read-only and Save is disabled; without `::read` a permission-denied alert.
 * Mirrors `SandboxResourceLimitsSection`.
 */
export function JsToolSettingsSection() {
  const { settings, loading, saving, error } = JsToolSettingsStore
  const canManage = usePermission(MANAGE_PERM)
  const canRead = usePermission(READ_PERM) || canManage

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: EMPTY_DEFAULTS,
  })
  const [dirty, setDirty] = useState(false)

  useEffect(() => {
    if (settings) {
      form.reset(rowToForm(settings))
      setDirty(false)
    }
  }, [settings, form])

  useEffect(() => {
    const sub = form.watch(() => setDirty(true))
    return () => sub.unsubscribe()
  }, [form])

  const onSubmit = async (v: FormValues) => {
    try {
      await JsToolSettingsStore.saveSettings(formToPatch(v))
      message.success('run_js limits saved')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save')
    }
  }

  const onReset = () => {
    if (settings) {
      form.reset(rowToForm(settings))
      setDirty(false)
    }
  }

  if (!canRead) {
    return (
      <Card title="Resource limits" data-testid="js-tool-settings-card">
        <Alert
          tone="warning"
          title="You don't have permission to view run_js limits."
          data-testid="js-tool-settings-noperm-alert"
        />
      </Card>
    )
  }

  return (
    <>
      {error && (
        <Alert
          tone="error"
          title="Failed to load run_js limits"
          description={error}
          data-testid="js-tool-settings-error-alert"
        />
      )}

      {loading && !settings ? (
        <Spin label="Loading run_js limits…" description="Loading run_js limits…" />
      ) : (
        <Card
          title="Resource limits"
          data-testid="js-tool-settings-card"
          footer={
            <SettingsFormActions
              onSave={form.handleSubmit(onSubmit)}
              onCancel={onReset}
              saving={saving}
              saveDisabled={!canManage || !dirty}
              cancelDisabled={!dirty || saving}
              cancelLabel="Reset"
              saveTestid="js-tool-settings-save-btn"
              cancelTestid="js-tool-settings-reset-btn"
            />
          }
        >
          <Form
            form={form}
            layout="horizontal"
            onSubmit={onSubmit}
            disabled={!canManage}
            data-testid="js-tool-settings-form"
          >
            {!canManage && (
              <Alert
                tone="info"
                title="Read-only view"
                description="You have read permission for run_js limits but not manage. Save is disabled."
                data-testid="js-tool-settings-readonly-alert"
              />
            )}

            <Separator titlePlacement="left">
              <Text type="secondary" className="text-xs">
                Interpreter
              </Text>
            </Separator>
            <FormField
              name="memory_mib"
              label="Memory limit (MiB)"
              description="Per-run heap cap for the embedded QuickJS interpreter (set_memory_limit). Allocating past it aborts the script with a memory error."
            >
              <InputNumber min={16} max={4096} suffix="MiB" className="w-full" data-testid="js-tool-memory" />
            </FormField>
            <FormField
              name="max_stack_kib"
              label="Stack size (KiB)"
              description="Per-run JS stack cap (set_max_stack_size). Deep recursion past it aborts the script."
            >
              <InputNumber min={64} max={65_536} suffix="KiB" className="w-full" data-testid="js-tool-stack" />
            </FormField>

            <Separator titlePlacement="left">
              <Text type="secondary" className="text-xs">
                Timeouts
              </Text>
            </Separator>
            <FormField
              name="wall_secs"
              label="Wall-clock timeout (seconds)"
              description="Active-execution budget for one run_js (excludes time spent awaiting a user approval). Trips a cancel that the interpreter observes."
            >
              <InputNumber min={1} max={3600} suffix="s" className="w-full" data-testid="js-tool-wall" />
            </FormField>
            <FormField
              name="approval_timeout_secs"
              label="Approval timeout (seconds)"
              description="How long a gated sub-tool call waits for the user to approve/deny before resolving as cancel."
            >
              <InputNumber min={5} max={3600} suffix="s" className="w-full" data-testid="js-tool-approval-timeout" />
            </FormField>

            <Separator titlePlacement="left">
              <Text type="secondary" className="text-xs">
                Concurrency &amp; trace
              </Text>
            </Separator>
            <FormField
              name="max_concurrent_runs"
              label="Max concurrent runs"
              description="Server-wide cap on simultaneous run_js interpreters (admission control). A burst past it fails fast with a 'busy' result. Applies immediately on save."
            >
              <InputNumber min={1} max={256} className="w-full" data-testid="js-tool-max-runs" />
            </FormField>
            <FormField
              name="max_concurrent_dispatch"
              label="Max concurrent sub-tool dispatch"
              description="Per-run cap on parallel sub-tool calls a single script may have in flight (Promise.all fan-out)."
            >
              <InputNumber min={1} max={64} className="w-full" data-testid="js-tool-max-dispatch" />
            </FormField>
            <FormField
              name="max_trace_entries"
              label="Max trace entries"
              description="Per-run cap on recorded sub-call trace entries surfaced in the result's structured content."
            >
              <InputNumber min={1} max={10_000} className="w-full" data-testid="js-tool-max-trace" />
            </FormField>

            <Paragraph type="secondary" className="mt-6">
              Defaults: 128 MiB memory, 512 KiB stack, 300 s wall-clock, 300 s
              approval, 8 concurrent runs, 6 concurrent dispatch, 256 trace
              entries. Values stored at <code>js_tool_settings</code>; the server
              invalidates its in-process cache on save.
            </Paragraph>
          </Form>
        </Card>
      )}
    </>
  )
}
