import { useEffect } from 'react'
import {
  Alert,
  Card,
  Form,
  FormField,
  InputNumber,
  Paragraph,
  message,
  useForm,
  zodResolver,
} from '@ziee/kit'
import { z } from 'zod'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/types'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

const schema = z.object({
  kb_max_documents: z.number().int().min(1).max(100000),
  search_max_hit_chars: z.number().int().min(100).max(100000),
  search_snippet_chars: z.number().int().min(20).max(4000),
  search_max_top_k: z.number().int().min(1).max(500),
})
type FormValues = z.infer<typeof schema>

/**
 * Retrieval limits — knobs that were previously compiled-in constants, now
 * admin-configurable on the shared Document-RAG settings row (used by the
 * Knowledge Base module too). Defaults preserve prior behaviour exactly.
 */
export function RetrievalLimitsSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving, error } = Stores.FileRagAdmin

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      kb_max_documents: 2000,
      search_max_hit_chars: 2000,
      search_snippet_chars: 160,
      search_max_top_k: 50,
    },
  })

  useEffect(() => {
    if (settings && !form.formState.isDirty) {
      form.reset({
        kb_max_documents: settings.kb_max_documents,
        search_max_hit_chars: settings.search_max_hit_chars,
        search_snippet_chars: settings.search_snippet_chars,
        search_max_top_k: settings.search_max_top_k,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card data-testid="filerag-limits-card" title="Retrieval limits">
        <Alert
          data-testid="filerag-limits-noperm-alert"
          tone="warning"
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Retrieval limits"
        error={error}
        onRetry={() => Stores.FileRagAdmin.load()}
      />
    )

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.FileRagAdmin.update({
        kb_max_documents: values.kb_max_documents,
        search_max_hit_chars: values.search_max_hit_chars,
        search_snippet_chars: values.search_snippet_chars,
        search_max_top_k: values.search_max_top_k,
      })
      message.success('Retrieval limits saved.')
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Failed to save settings.')
    }
  }

  return (
    <Card
      data-testid="filerag-limits-card"
      title="Retrieval limits"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSubmit)}
          onCancel={() => form.reset()}
          saving={saving}
          saveTestid="filerag-limits-save"
          cancelTestid="filerag-limits-cancel"
        />
      ) : undefined}
    >
      {error && (
        <Alert data-testid="filerag-limits-error-alert" tone="error" className="!mb-4" title={error} />
      )}
      <Paragraph type="secondary" className="!mb-3 text-sm">
        Caps for knowledge-base documents and search-result size. Applied
        deployment-wide; a running search clamps a requested top-k to the ceiling below.
      </Paragraph>
      <Form
        data-testid="filerag-limits-form"
        name="file-rag-admin-limits-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSubmit}
        disabled={!canManage}
      >
        <FormField
          name="kb_max_documents"
          label="Max documents per knowledge base"
          description="Hard cap on how many documents one knowledge base can hold; adding beyond it is rejected (422)."
        >
          <InputNumber data-testid="filerag-limits-kb-max-docs" min={1} max={100000} step={100} className="w-40" />
        </FormField>

        <FormField
          name="search_max_top_k"
          label="Max passages per search (ceiling)"
          description="The hard upper bound a requested top-k is clamped to for search_knowledge."
        >
          <InputNumber data-testid="filerag-limits-max-top-k" min={1} max={500} step={1} className="w-40" />
        </FormField>

        <FormField
          name="search_max_hit_chars"
          label="Max characters per returned passage"
          description="Each retrieved passage's text is truncated to this length before it reaches the model."
        >
          <InputNumber data-testid="filerag-limits-max-hit-chars" min={100} max={100000} step={100} className="w-40" />
        </FormField>

        <FormField
          name="search_snippet_chars"
          label="Snippet length (text summary)"
          description="How many characters of each passage appear in the human-readable result summary."
        >
          <InputNumber data-testid="filerag-limits-snippet-chars" min={20} max={4000} step={20} className="w-40" />
        </FormField>
      </Form>
    </Card>
  )
}
