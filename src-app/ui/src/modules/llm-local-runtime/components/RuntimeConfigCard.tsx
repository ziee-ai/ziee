import { useEffect } from 'react'
import { z } from 'zod'
import {
  Card,
  Form,
  FormField,
  useForm,
  zodResolver,
  InputNumber,
  Spin,
  message,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/types'

const schema = z.object({
  idle_unload_secs: z.number().min(0).max(86400),
  auto_start_timeout_secs: z.number().min(1).max(600),
  drain_timeout_secs: z.number().min(1).max(600),
})
type Schema = z.infer<typeof schema>

/**
 * Runtime config card: the singleton llm_runtime_settings row —
 * idle_unload_secs / auto_start_timeout_secs / drain_timeout_secs.
 * Mirrors the peer settings module layout (Text strong section header
 * + secondary description + Form.Item; Save in a justify-end flex
 * after a Divider).
 */
export function RuntimeConfigCard() {
  const { settings, loadingSettings, savingSettings, error } =
    Stores.RuntimeConfig
  const canManage = usePermission(Permissions.RuntimeSettingsManage)
  const form = useForm<Schema>({
    resolver: zodResolver(schema),
    defaultValues: {
      idle_unload_secs: 0,
      auto_start_timeout_secs: 30,
      drain_timeout_secs: 30,
    },
  })

  useEffect(() => {
    if (settings) {
      form.reset({
        idle_unload_secs: settings.idle_unload_secs,
        auto_start_timeout_secs: settings.auto_start_timeout_secs,
        drain_timeout_secs: settings.drain_timeout_secs,
      })
    }
  }, [settings, form])

  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.RuntimeConfig.clearError()
    }
  }, [error])

  const handleSave = async (values: Schema) => {
    try {
      await Stores.RuntimeConfig.saveSettings(values)
      message.success('Runtime settings saved')
    } catch {
      // validation / save error already surfaced via the error effect
    }
  }

  if (loadingSettings && !settings) {
    return (
      <Card title="Runtime configuration" data-testid="llmrt-runtime-config-card">
        <Spin label="Loading" />
      </Card>
    )
  }

  return (
    <Card
      title="Runtime configuration"
      data-testid="llmrt-runtime-config-card"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSave)}
          onCancel={() => form.reset()}
          saving={savingSettings}
          saveTestid="llmrt-config-save-btn"
          cancelTestid="llmrt-config-cancel-btn"
        />
      ) : undefined}
    >
      <Form
        form={form}
        onSubmit={handleSave}
        disabled={!canManage}
        data-testid="llmrt-runtime-config-form"
        // Two columns: label on the left, input + help text on the
        // right. xs (mobile) collapses to stacked (label on top of
        // input) so neither side gets squeezed below a usable width.
        layout="horizontal"
      >
        <FormField
          name="idle_unload_secs"
          label="Idle unload timeout (seconds)"
          description="Engines idle longer than this are automatically unloaded to free memory. 0 disables idle eviction."
          required
        >
          <InputNumber min={0} max={86400} className="!w-full" data-testid="llmrt-config-idle-unload" />
        </FormField>

        <FormField
          name="auto_start_timeout_secs"
          label="Auto-start timeout (seconds)"
          description="How long the proxy waits for a freshly-spawned engine to become healthy before giving up."
          required
        >
          <InputNumber min={1} max={600} className="!w-full" data-testid="llmrt-config-autostart-timeout" />
        </FormField>

        <FormField
          name="drain_timeout_secs"
          label="Drain timeout (seconds)"
          description="When unloading an idle engine, how long to wait for in-flight requests to finish before forcing the stop."
          required
        >
          <InputNumber min={1} max={600} className="!w-full" data-testid="llmrt-config-drain-timeout" />
        </FormField>

      </Form>
    </Card>
  )
}
