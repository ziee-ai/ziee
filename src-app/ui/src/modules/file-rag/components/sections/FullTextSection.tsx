import { useEffect } from 'react'
import {
  Alert,
  Card,
  Form,
  FormField,
  InputNumber,
  Switch,
  message,
  useForm,
  zodResolver,
} from '@ziee/kit'
import { z } from 'zod'
import { Stores } from '@/core/stores'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

interface FormValues {
  fts_enabled: boolean
  fts_rrf_k: number
  fts_candidate_multiplier: number
  fts_min_rank: number
}

const schema = z.object({
  fts_enabled: z.boolean(),
  fts_rrf_k: z.number(),
  fts_candidate_multiplier: z.number(),
  fts_min_rank: z.number(),
})

/**
 * Full-text (lexical) arm tuning. Works with no embedding model — this is the
 * day-one search experience. When semantic search is also on, the two arms are
 * fused with Reciprocal Rank Fusion (`fts_rrf_k`, `fts_candidate_multiplier`).
 */
export function FullTextSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving, error } = Stores.FileRagAdmin
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      fts_enabled: false,
      fts_rrf_k: 60,
      fts_candidate_multiplier: 5,
      fts_min_rank: 0,
    },
  })

  useEffect(() => {
    // Don't clobber the admin's unsaved edits on a mid-edit refetch.
    if (settings && !form.formState.isDirty) {
      form.reset({
        fts_enabled: settings.fts_enabled,
        fts_rrf_k: settings.fts_rrf_k,
        fts_candidate_multiplier: settings.fts_candidate_multiplier,
        fts_min_rank: settings.fts_min_rank,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card data-testid="filerag-fts-card" title="Full-text search">
        <Alert
          data-testid="filerag-fts-noperm-alert"
          tone="warning"
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Full-text search"
        error={error}
        onRetry={() => Stores.FileRagAdmin.load()}
      />
    )

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.FileRagAdmin.update({
        fts_enabled: values.fts_enabled,
        fts_rrf_k: values.fts_rrf_k,
        fts_candidate_multiplier: values.fts_candidate_multiplier,
        fts_min_rank: values.fts_min_rank,
      })
      message.success('Full-text settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save settings.',
      )
    }
  }

  return (
    <Card
      data-testid="filerag-fts-card"
      title="Full-text search"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSubmit)}
          onCancel={() => form.reset()}
          saving={saving}
          saveTestid="filerag-fts-save"
          cancelTestid="filerag-fts-cancel"
        />
      ) : undefined}
    >
      {error && <Alert data-testid="filerag-fts-error-alert" tone="error" className="!mb-4" title={error} />}
      <Form
        data-testid="filerag-fts-form"
        name="file-rag-admin-fts-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSubmit}
        disabled={!canManage}
      >
        <FormField
          name="fts_enabled"
          label="Enable full-text search"
          description="The lexical arm. When off (and no embedder is set), semantic_search returns nothing."
          valuePropName="checked"
        >
          <Switch data-testid="filerag-fts-switch" aria-label="Enable full-text search" />
        </FormField>

        <FormField
          name="fts_rrf_k"
          label="RRF k"
          description="Reciprocal Rank Fusion constant for blending the vector + full-text arms. Higher = more egalitarian. Default 60 (the RRF paper)."
        >
          <InputNumber data-testid="filerag-fts-rrf-k" min={1} max={1000} className="w-40" />
        </FormField>

        <FormField
          name="fts_candidate_multiplier"
          label="Candidate multiplier"
          description="Hybrid pulls top-K × this many candidates from each arm before fusion. Higher = more recall, more DB load."
        >
          <InputNumber data-testid="filerag-fts-candidate-mult" min={1} max={20} className="w-40" />
        </FormField>

        <FormField
          name="fts_min_rank"
          label="Minimum rank"
          description="ts_rank_cd cutoff. 0.0 = no filter (default). Raise to drop weak lexical matches."
        >
          <InputNumber data-testid="filerag-fts-min-rank" min={0} max={1} step={0.05} className="w-40" />
        </FormField>

      </Form>
    </Card>
  )
}
