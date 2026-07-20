import { useEffect } from 'react'
import {
  Alert,
  Card,
  Form,
  FormField,
  useForm,
  zodResolver,
  InputNumber,
  message,
} from '@ziee/kit'
import { z } from 'zod'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'
import { Permissions } from '@/api-client/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

const schema = z.object({
  soft_delete_grace_days: z.number().min(1).max(365),
  daily_extraction_quota: z.number().min(1).max(10000),
})

type FormValues = z.infer<typeof schema>

/**
 * Retention + extraction quota. Own form.
 */
export function RetentionLimitsSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving, error } = Stores.MemoryAdmin
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      soft_delete_grace_days: 30,
      daily_extraction_quota: 100,
    },
  })

  useEffect(() => {
    if (settings) {
      form.reset({
        soft_delete_grace_days: settings.soft_delete_grace_days,
        daily_extraction_quota: settings.daily_extraction_quota,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Retention & extraction limits" data-testid="memory-retention-card">
        <Alert
          tone="warning"
          title="You don't have permission to view memory admin settings."
          data-testid="memory-retention-no-perm-alert"
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Retention & extraction limits"
        error={error}
        onRetry={() => Stores.MemoryAdmin.load()}
      />
    )

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.MemoryAdmin.update({
        soft_delete_grace_days: values.soft_delete_grace_days,
        daily_extraction_quota: values.daily_extraction_quota,
      })
      message.success('Retention & limits saved.')
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to save retention settings.',
      )
    }
  }

  return (
    <Card
      title="Retention & extraction limits"
      data-testid="memory-retention-card"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSubmit)}
          onCancel={() => form.reset()}
          saving={saving}
          saveTestid="memory-retention-save-btn"
          cancelTestid="memory-retention-cancel-btn"
        />
      ) : undefined}
    >
      <Form
        name="memory-admin-retention-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSubmit}
        disabled={!canManage}
        data-testid="memory-retention-form"
      >
        <FormField
          name="soft_delete_grace_days"
          label="Soft-delete grace days"
          description="How long soft-deleted memories stick around before the nightly reaper hard-deletes them. Lower = faster GDPR/erasure compliance; higher = longer audit window for user-initiated undeletes."
        >
          <InputNumber min={1} max={365} className="w-40" data-testid="memory-retention-grace-input" />
        </FormField>
        <FormField
          name="daily_extraction_quota"
          label="Daily extraction quota (per user)"
          description="Brake against extraction-spam loops. When a user hits this many extraction-sourced memories in a 24h window, further extraction is skipped silently. The hard cost gate is your LLM API spend; this is the secondary brake on row count."
        >
          <InputNumber min={1} max={10000} className="w-40" data-testid="memory-retention-quota-input" />
        </FormField>

      </Form>
    </Card>
  )
}
