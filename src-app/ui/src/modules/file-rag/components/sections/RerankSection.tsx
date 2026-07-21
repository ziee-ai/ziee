import { useEffect } from 'react'
import {
  Alert,
  Card,
  Combobox,
  Form,
  FormField,
  InputNumber,
  Paragraph,
  Switch,
  message,
  useForm,
  zodResolver,
} from '@ziee/kit'
import { z } from 'zod'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/permissions'
import { FileRagAdmin } from '@/modules/file-rag/stores/fileRagAdmin'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

interface FormValues {
  rerank_enabled: boolean
  reranker_model_id?: string | null
  rerank_candidate_k: number
}

const schema = z.object({
  rerank_enabled: z.boolean(),
  reranker_model_id: z.string().nullable().optional(),
  rerank_candidate_k: z.number().int().min(1).max(200),
})

/**
 * Cross-encoder reranker: retrieve-wide → rerank → top-k. Improves retrieval
 * quality by re-scoring the top candidate passages against the query. OFF by
 * default; needs a model with the `rerank` capability (get one from the Hub).
 */
export function RerankSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, rerankerModels, saving, error } = FileRagAdmin

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      rerank_enabled: false,
      reranker_model_id: null,
      rerank_candidate_k: 30,
    },
  })

  useEffect(() => {
    if (settings && !form.formState.isDirty) {
      form.reset({
        rerank_enabled: settings.rerank_enabled,
        reranker_model_id: settings.reranker_model_id ?? null,
        rerank_candidate_k: settings.rerank_candidate_k,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card data-testid="filerag-rerank-card" title="Reranker">
        <Alert
          data-testid="filerag-rerank-noperm-alert"
          tone="warning"
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Reranker"
        error={error}
        onRetry={() => FileRagAdmin.load()}
      />
    )

  const noModels = rerankerModels.length === 0
  const modelOptions = [
    { value: '', label: 'None' },
    ...rerankerModels.map(m => ({
      value: m.id,
      label: m.display_name || m.name,
    })),
  ]

  const persist = async (values: FormValues) => {
    try {
      await FileRagAdmin.update({
        rerank_enabled: values.rerank_enabled,
        reranker_model_id: values.reranker_model_id ? values.reranker_model_id : null,
        rerank_candidate_k: values.rerank_candidate_k,
      })
      message.success('Reranker settings saved.')
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Failed to save settings.')
    }
  }

  return (
    <Card data-testid="filerag-rerank-card" title="Reranker">
      <Paragraph tone="secondary" className="!mb-3">
        A cross-encoder reranker re-scores the top candidate passages against the
        query so the most relevant chunks reach the model. Retrieve-wide → rerank
        → top-k. Optional; improves retrieval quality.
      </Paragraph>

      {noModels && (
        <Alert
          data-testid="filerag-rerank-hub-nudge"
          tone="info"
          className="!mb-3"
          title="No reranker model installed"
          description="Get BGE-reranker-v2-m3 from the Hub (browse Models → filter Reranker), then select it here to improve retrieval quality."
        />
      )}

      <Form
        data-testid="filerag-rerank-form"
        form={form}
        layout="vertical"
        disabled={!canManage}
        onSubmit={persist}
      >
        <FormField
          name="reranker_model_id"
          label="Reranker model"
          description="A model with the `rerank` capability (served locally via llama.cpp)."
        >
          <Combobox
            data-testid="filerag-rerank-model-combobox"
            options={modelOptions}
            placeholder="Select a reranker model"
            emptyText="No reranker models — install one from the Hub."
          />
        </FormField>

        <FormField
          name="rerank_enabled"
          label="Enable reranking"
          description="When on (and a model is selected), search reranks the candidate pool before returning top-k."
          valuePropName="checked"
        >
          <Switch
            data-testid="filerag-rerank-enable-switch"
            aria-label="Enable reranking"
          />
        </FormField>

        <FormField
          name="rerank_candidate_k"
          label="Candidate pool size"
          description="How many hits to retrieve before reranking down to top-k (1–200). Larger = better recall, slower."
        >
          <InputNumber
            data-testid="filerag-rerank-candidate-k-input"
            min={1}
            max={200}
          />
        </FormField>

        {canManage && (
          <SettingsFormActions
            onSave={form.handleSubmit(persist)}
            onCancel={() => form.reset()}
            saving={saving}
            saveTestid="filerag-rerank-save"
            cancelTestid="filerag-rerank-cancel"
          />
        )}
      </Form>
    </Card>
  )
}
