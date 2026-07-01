import { useEffect, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Form,
  FormField,
  InputNumber,
  Dialog,
  Combobox,
  Switch,
  Paragraph,
  useForm,
  zodResolver,
  message,
} from '@/components/ui'
import { z } from 'zod'
import { RotateCw } from 'lucide-react'
import { Stores } from '@/core/stores'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

interface FormValues {
  semantic_enabled: boolean
  embedding_model_id?: string | null
  cosine_threshold: number
}

const schema = z.object({
  semantic_enabled: z.boolean(),
  embedding_model_id: z.string().nullable().optional(),
  cosine_threshold: z.number(),
})

/**
 * Embedding (vector) arm: the `semantic_enabled` kill switch, the
 * embedding-model picker (dimension is derived by probing the model
 * server-side), the cosine-distance cutoff, and a "Re-embed now" affordance.
 *
 * Effective semantic recall requires `semantic_enabled AND embedding_model_id`.
 * Setting/changing the model re-embeds the whole corpus in the background.
 */
export function EmbeddingSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const {
    settings,
    embeddingModels,
    saving,
    loadingModels,
    triggeringReembed,
    error,
  } = Stores.FileRagAdmin
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      semantic_enabled: false,
      embedding_model_id: null,
      cosine_threshold: 0,
    },
  })
  const [reembedConfirmOpen, setReembedConfirmOpen] = useState(false)
  const [pendingSwap, setPendingSwap] = useState<FormValues | null>(null)

  useEffect(() => {
    // Don't clobber the admin's unsaved edits on a mid-edit refetch.
    if (settings && !form.formState.isDirty) {
      form.reset({
        semantic_enabled: settings.semantic_enabled,
        embedding_model_id: settings.embedding_model_id,
        cosine_threshold: settings.cosine_threshold,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card data-testid="filerag-embedding-card" title="Embedding (semantic search)">
        <Alert
          data-testid="filerag-embedding-noperm-alert"
          tone="warning"
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Embedding (semantic search)"
        error={error}
        onRetry={() => Stores.FileRagAdmin.load()}
      />
    )

  const noModelsAvailable = embeddingModels.length === 0

  const persist = async (values: FormValues, modelChanged: boolean) => {
    try {
      await Stores.FileRagAdmin.update({
        semantic_enabled: values.semantic_enabled,
        embedding_model_id: values.embedding_model_id ?? null,
        cosine_threshold: values.cosine_threshold,
      })
      message.success(
        modelChanged
          ? 'Saved. Embedding model changed — re-embed running in the background.'
          : 'Embedding settings saved.',
      )
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save settings.',
      )
    }
  }

  const handleSubmit = async (values: FormValues) => {
    const priorId = settings.embedding_model_id
    const newId = values.embedding_model_id ?? null
    const modelChanged = newId !== priorId
    // Swapping between two different models re-embeds everything — confirm.
    if (modelChanged && newId !== null && priorId !== null) {
      setPendingSwap(values)
      return
    }
    await persist(values, modelChanged)
  }

  const handleReembed = async () => {
    if (!settings.embedding_model_id) return
    setReembedConfirmOpen(false)
    try {
      await Stores.FileRagAdmin.triggerReembed()
      message.info('Re-embed dispatched in the background.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to start re-embed.',
      )
    }
  }

  const swapTargetLabel = pendingSwap
    ? (embeddingModels.find(m => m.id === pendingSwap.embedding_model_id)
        ?.display_name ??
      embeddingModels.find(m => m.id === pendingSwap.embedding_model_id)?.name ??
      pendingSwap.embedding_model_id)
    : ''

  return (
    <>
      <Card
        data-testid="filerag-embedding-card"
        title="Embedding (semantic search)"
        footer={canManage ? (
          <SettingsFormActions
            onSave={form.handleSubmit(handleSubmit)}
            onCancel={() => form.reset()}
            saving={saving}
            saveTestid="filerag-embedding-save"
            cancelTestid="filerag-embedding-cancel"
          />
        ) : undefined}
      >
        {error && <Alert data-testid="filerag-embedding-error-alert" tone="error" className="!mb-4" title={error} />}
        {noModelsAvailable && (
          <Alert
            data-testid="filerag-embedding-no-models-alert"
            tone="info"
            className="!mb-4"
            title="No embedding-capable models found."
            description={
              <span>
                Add one from the LLM Providers page (upload a GGUF such as{' '}
                <code>nomic-embed-text-v1.5</code>, or register a remote model
                like <code>text-embedding-3-small</code>) and tick the{' '}
                <strong>text_embedding</strong> capability. Until then, document
                search runs in full-text mode.
              </span>
            }
          />
        )}
        <Form
          data-testid="filerag-embedding-form"
          name="file-rag-admin-embedding-form"
          form={form}
          layout="horizontal"
          onSubmit={handleSubmit}
          disabled={!canManage}
        >
          <FormField
            name="semantic_enabled"
            label="Enable semantic search"
            description="When off, retrieval uses full-text only. Effective semantic recall also requires an embedding model below."
            valuePropName="checked"
          >
            <Switch data-testid="filerag-embedding-switch" aria-label="Enable semantic search" />
          </FormField>

          <FormField
            name="embedding_model_id"
            label="Embedding model"
            description={`The model used to compute document + query vectors. The vector dimension is derived automatically from the model. Current dimension: ${settings.embedding_dimensions}`}
          >
            <Combobox
              data-testid="filerag-embedding-model-select"
              placeholder={
                noModelsAvailable
                  ? 'No embedding-capable models'
                  : 'Select an embedding model'
              }
              searchPlaceholder="Search models"
              emptyText="No embedding-capable models"
              loading={loadingModels}
              disabled={noModelsAvailable}
              options={embeddingModels.map(m => ({
                value: m.id,
                label: m.display_name || m.name,
              }))}
              className="max-w-[480px]"
            />
          </FormField>

          <FormField
            name="cosine_threshold"
            label="Cosine distance threshold"
            description="Chunks with distance ≥ this value are dropped from the vector arm. Lower = stricter."
          >
            <InputNumber data-testid="filerag-embedding-cosine" min={0} max={2} step={0.05} className="w-40" />
          </FormField>

          {/* No form field — a labeled action row (Form.Item without a name). */}
          <div className="mb-4">
            <div className="text-sm font-medium mb-1">
              Force re-embed all chunks
            </div>
            <Button
              data-testid="filerag-embedding-reembed-btn"
              icon={<RotateCw />}
              loading={triggeringReembed}
              onClick={() => setReembedConfirmOpen(true)}
              disabled={!settings.embedding_model_id || !canManage}
            >
              Re-embed now
            </Button>
            <div className="text-muted-foreground text-sm mt-1">
              Re-embeds every stored chunk with the current model. Useful after
              an embedder upgrade.
            </div>
          </div>

        </Form>
      </Card>

      <Dialog
        data-testid="filerag-embedding-reembed-dialog"
        open={reembedConfirmOpen}
        onOpenChange={v => {
          if (!v) setReembedConfirmOpen(false)
        }}
        title="Re-embed every document chunk?"
        footer={
          <>
            <Button data-testid="filerag-embedding-reembed-cancel" variant="outline" onClick={() => setReembedConfirmOpen(false)}>
              Cancel
            </Button>
            <Button data-testid="filerag-embedding-reembed-confirm" onClick={handleReembed}>Re-embed</Button>
          </>
        }
      >
        <Paragraph>
          This runs in the background. Semantic search skips not-yet-embedded
          chunks (full-text still works) and catches up as the worker fills them
          in. For large corpora this can take several minutes.
        </Paragraph>
      </Dialog>

      <Dialog
        data-testid="filerag-embedding-swap-dialog"
        open={pendingSwap !== null}
        onOpenChange={v => {
          if (!v) setPendingSwap(null)
        }}
        title="Change the embedding model?"
        footer={
          <>
            <Button data-testid="filerag-embedding-swap-cancel" variant="outline" onClick={() => setPendingSwap(null)}>
              Keep current model
            </Button>
            <Button
              data-testid="filerag-embedding-swap-confirm"
              onClick={async () => {
                if (!pendingSwap) return
                const captured = pendingSwap
                setPendingSwap(null)
                await persist(captured, true)
              }}
            >
              Change and re-embed
            </Button>
          </>
        }
      >
        <Paragraph>
          Switching to <code>{swapTargetLabel}</code> re-computes every stored
          chunk's embedding in the background (and rebuilds the vector index if
          the dimension differs).
        </Paragraph>
        <Paragraph type="secondary" className="!mb-0 text-sm">
          During the rebuild, semantic search returns fewer results; full-text
          search is unaffected. New uploads are picked up automatically.
        </Paragraph>
      </Dialog>
    </>
  )
}
