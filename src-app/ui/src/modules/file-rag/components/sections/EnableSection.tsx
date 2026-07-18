import { useEffect } from 'react'
import { Alert, Card, Form, FormField, InputNumber, Switch, message, useForm, zodResolver } from '@ziee/kit'
import { z } from 'zod'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/types'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

const schema = z.object({
  enabled: z.boolean(),
  default_top_k: z.number().int().min(1).max(50),
})

type FormValues = z.infer<typeof schema>

/**
 * Master Document-RAG card: deployment-wide enable + the shared `default_top_k`
 * retrieval cap. Default is ON (full-text from day one). Per-arm toggles live
 * in their own cards below.
 */
export function EnableSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving, error } = Stores.FileRagAdmin
  const form = useForm<FormValues>({ resolver: zodResolver(schema) })

  useEffect(() => {
    // Don't clobber the admin's unsaved edits when a refetch (e.g. a sync
    // reconnect) reloads settings mid-edit.
    if (settings && !form.formState.isDirty) {
      form.reset({
        enabled: settings.enabled,
        default_top_k: settings.default_top_k,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card data-testid="filerag-enable-card" title="Document search">
        <Alert
          data-testid="filerag-enable-noperm-alert"
          tone="warning"
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Document search"
        error={error}
        onRetry={() => Stores.FileRagAdmin.load()}
      />
    )

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.FileRagAdmin.update({
        enabled: values.enabled,
        default_top_k: values.default_top_k,
      })
      message.success('Document search settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save settings.',
      )
    }
  }

  return (
    <Card
      data-testid="filerag-enable-card"
      title="Document search"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSubmit)}
          onCancel={() => form.reset()}
          saving={saving}
          saveTestid="filerag-enable-save"
          cancelTestid="filerag-enable-cancel"
        />
      ) : undefined}
    >
      {error && <Alert data-testid="filerag-enable-error-alert" tone="error" className="!mb-4" title={error} />}
      <Form
        data-testid="filerag-enable-form"
        name="file-rag-admin-master-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSubmit}
        disabled={!canManage}
      >
        <FormField
          name="enabled"
          label="Enable Document RAG deployment-wide"
          description="On by default. When off, files are not indexed and the semantic_search tool returns a disabled note. Full-text search works immediately; semantic search additionally needs an embedding model (below)."
          valuePropName="checked"
        >
          <Switch data-testid="filerag-enable-switch" aria-label="Enable Document RAG deployment-wide" />
        </FormField>

        <FormField
          name="default_top_k"
          label="Default top-K"
          description="How many passages semantic_search returns when the caller doesn't specify. The model can request fewer per call; a single call returns at most 50."
        >
          <InputNumber data-testid="filerag-enable-top-k" min={1} max={50} className="w-40" />
        </FormField>

      </Form>
    </Card>
  )
}
