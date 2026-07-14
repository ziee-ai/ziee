import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Form,
  FormField,
  useForm,
  zodResolver,
  InputNumber,
  Switch,
  message,
} from '@ziee/kit'
import { z } from 'zod'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

const schema = z.object({
  enabled: z.boolean(),
  default_top_k: z.number().min(1).max(100),
})
type FormValues = z.infer<typeof schema>

/**
 * Master memory card: deployment-wide kill switch + the shared
 * `default_top_k` retrieval cap. Per-arm enable toggles
 * (`fts_enabled`, `semantic_enabled`) live in their own cards below.
 */
export function MemorySection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving, error } = Stores.MemoryAdmin
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { enabled: false, default_top_k: 10 },
  })

  useEffect(() => {
    if (settings) {
      form.reset({
        enabled: settings.enabled,
        default_top_k: settings.default_top_k,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Memory" data-testid="memory-admin-master-card">
        <Alert
          tone="warning"
          title="You don't have permission to view memory admin settings."
          data-testid="memory-admin-no-perm-alert"
        />
      </Card>
    )
  }
  if (!settings && error) {
    return (
      <Card title="Memory" data-testid="memory-admin-master-card">
        <Alert
          data-testid="memory-section-load-error-alert"
          tone="error"
          title="Failed to load memory settings"
          description={error}
        >
          <Button data-testid="memory-section-retry-btn" size="default" onClick={() => Stores.MemoryAdmin.load()}>
            Retry
          </Button>
        </Alert>
      </Card>
    )
  }
  if (!settings) return null

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.MemoryAdmin.update({
        enabled: values.enabled,
        default_top_k: values.default_top_k,
      })
      message.success('Memory settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save memory settings.',
      )
    }
  }

  return (
    <Card
      title="Memory"
      data-testid="memory-admin-master-card"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSubmit)}
          onCancel={() => form.reset()}
          saving={saving}
          saveTestid="memory-admin-master-save-btn"
          cancelTestid="memory-admin-master-cancel-btn"
        />
      ) : undefined}
    >
      <Form
        name="memory-admin-master-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSubmit}
        disabled={!canManage}
        data-testid="memory-admin-master-form"
      >
        <FormField
          name="enabled"
          label="Enable memory deployment-wide"
          description="When off, all memory hooks no-op silently. Per-user toggles are unaffected but have no effect until this is on."
          valuePropName="checked"
        >
          <Switch aria-label="Enable memory deployment-wide" data-testid="memory-admin-enabled-switch" />
        </FormField>

        <FormField
          name="default_top_k"
          label="Default top-K"
          description="How many memories to inject per turn. Shared across retrieval arms — the fused top-K is what's injected, whether the result came from full-text, semantic, or hybrid search. Users can override their own limit later."
        >
          <InputNumber min={1} max={100} className="w-40" data-testid="memory-admin-topk-input" />
        </FormField>

      </Form>
    </Card>
  )
}
