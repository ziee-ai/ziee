import { useEffect } from 'react'
import {
  Alert,
  Card,
  Form,
  FormField,
  useForm,
  zodResolver,
  Combobox,
  message,
} from '@/components/ui'
import { z } from 'zod'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

const schema = z.object({
  default_extraction_model_id: z.string().nullable().optional(),
})
type FormValues = z.infer<typeof schema>

/**
 * Memory extraction admin card: which LLM the silent extraction pipeline
 * defaults to. Per-user override is possible (Preferences); the value
 * picked here is the fallback when a user hasn't set their own.
 */
export function ExtractionSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, availableModels, saving, loadingModels, error } =
    Stores.MemoryAdmin
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { default_extraction_model_id: null },
  })

  useEffect(() => {
    if (settings) {
      form.reset({
        default_extraction_model_id: settings.default_extraction_model_id,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Extraction" data-testid="memory-extraction-card">
        <Alert
          tone="warning"
          title="You don't have permission to view memory admin settings."
          data-testid="memory-extraction-no-perm-alert"
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Extraction"
        error={error}
        onRetry={() => Stores.MemoryAdmin.load()}
      />
    )

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.MemoryAdmin.update({
        default_extraction_model_id: values.default_extraction_model_id ?? null,
      })
      message.success('Extraction settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to save extraction settings.',
      )
    }
  }

  return (
    <Card
      title="Extraction"
      data-testid="memory-extraction-card"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSubmit)}
          onCancel={() => form.reset()}
          saving={saving}
          saveTestid="memory-extraction-save-btn"
          cancelTestid="memory-extraction-cancel-btn"
        />
      ) : undefined}
    >
      <Form
        name="memory-admin-extraction-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSubmit}
        disabled={!canManage}
        data-testid="memory-extraction-form"
      >
        <FormField
          name="default_extraction_model_id"
          label="Default extraction model"
          description="LLM used by the silent extraction pipeline. Users can override per-account. Cheap models (Haiku-class, Gemini Flash) are ideal here."
        >
          <Combobox
            data-testid="memory-extraction-model-combobox"
            placeholder={
              !loadingModels && availableModels.length === 0
                ? 'No chat-capable models — add one on the LLM Providers page'
                : 'Select an extraction model (optional)'
            }
            searchPlaceholder="Search models"
            emptyText="No models"
            loading={loadingModels}
            options={availableModels.map((m) => ({
              value: m.id,
              label: m.display_name || m.name,
            }))}
            className="max-w-[480px]"
          />
        </FormField>

      </Form>
    </Card>
  )
}
