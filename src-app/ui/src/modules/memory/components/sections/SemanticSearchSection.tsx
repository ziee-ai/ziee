import { useEffect, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Flex,
  Form,
  FormField,
  useForm,
  zodResolver,
  InputNumber,
  Dialog,
  Combobox,
  Switch,
  Paragraph,
  message,
} from '@ziee/kit'
import { z } from 'zod'
import { RotateCw } from 'lucide-react'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/permissions'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

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
 * Semantic (vector) search admin card. Owns the `semantic_enabled`
 * kill switch (migration 90), the embedding-model picker, the
 * cosine-distance cutoff, and the explicit "Re-embed now" affordance.
 *
 * Effective vector recall requires
 * `semantic_enabled AND embedding_model_id IS NOT NULL`.
 */
export function SemanticSearchSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, embeddingModels, saving, loadingModels, error } =
    Stores.MemoryAdmin
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      semantic_enabled: false,
      embedding_model_id: null,
      cosine_threshold: 1,
    },
  })
  const [reembedConfirmOpen, setReembedConfirmOpen] = useState(false)
  const [pendingSwap, setPendingSwap] = useState<FormValues | null>(null)

  useEffect(() => {
    if (settings) {
      form.reset({
        semantic_enabled: settings.semantic_enabled,
        embedding_model_id: settings.embedding_model_id,
        cosine_threshold: settings.cosine_threshold,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Semantic search" data-testid="memory-semantic-card">
        <Alert
          tone="warning"
          title="You don't have permission to view memory admin settings."
          data-testid="memory-semantic-no-perm-alert"
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Semantic search"
        error={error}
        onRetry={() => Stores.MemoryAdmin.load()}
      />
    )

  // Don't claim "no models" until the fetch has actually completed — otherwise
  // the warning + disabled select flash during the initial load.
  const noModelsAvailable = !loadingModels && embeddingModels.length === 0

  const persist = async (values: FormValues, modelChanged: boolean) => {
    try {
      await Stores.MemoryAdmin.update({
        semantic_enabled: values.semantic_enabled,
        embedding_model_id: values.embedding_model_id ?? null,
        cosine_threshold: values.cosine_threshold,
      })
      if (modelChanged) {
        message.success(
          'Semantic search saved. Embedding model changed — re-embed running in background.',
        )
        Stores.MemoryAdmin.loadRebuildStatus()
      } else {
        message.success('Semantic search saved.')
      }
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to save semantic search settings.',
      )
    }
  }

  const handleSubmit = async (values: FormValues) => {
    const priorEmbeddingId = settings.embedding_model_id
    const newEmbeddingId = values.embedding_model_id ?? null
    const modelChanged = newEmbeddingId !== priorEmbeddingId

    if (modelChanged && newEmbeddingId !== null && priorEmbeddingId !== null) {
      setPendingSwap(values)
      return
    }
    await persist(values, modelChanged)
  }

  const handleReembed = async () => {
    if (!settings.embedding_model_id) return
    setReembedConfirmOpen(false)
    try {
      await Stores.MemoryAdmin.triggerReembed()
      message.info(
        'Re-embed job dispatched in background. Retrieval temporarily reduced until complete.',
      )
      Stores.MemoryAdmin.loadRebuildStatus()
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to start re-embed job.',
      )
    }
  }

  const swapTargetLabel = pendingSwap
    ? (embeddingModels.find((m) => m.id === pendingSwap.embedding_model_id)
        ?.display_name ??
      embeddingModels.find((m) => m.id === pendingSwap.embedding_model_id)
        ?.name ??
      pendingSwap.embedding_model_id)
    : ''

  return (
    <>
      <Card
        title="Semantic search"
        data-testid="memory-semantic-card"
        footer={canManage ? (
          <SettingsFormActions
            onSave={form.handleSubmit(handleSubmit)}
            onCancel={() => form.reset()}
            saving={saving}
            saveTestid="memory-semantic-save-btn"
            cancelTestid="memory-semantic-cancel-btn"
          />
        ) : undefined}
      >
        {noModelsAvailable && (
          <Alert
            tone="info"
            className="!mb-4"
            title="No embedding-capable models found."
            data-testid="memory-semantic-no-models-alert"
            description={
              <span>
                Add one from the LLM Providers page — either upload a
                GGUF (e.g. <code>nomic-embed-text-v1.5</code>), download
                from HuggingFace, or register a remote API model like{' '}
                <code>text-embedding-3-small</code>. Tick the{' '}
                <strong>text_embedding</strong> capability on the model
                form. Then return here and select it below.
              </span>
            }
          />
        )}
        <Form
          name="memory-admin-semantic-form"
          form={form}
          layout="horizontal"
          onSubmit={handleSubmit}
          disabled={!canManage}
          data-testid="memory-semantic-form"
        >
          <FormField
            name="semantic_enabled"
            label="Enable semantic search"
            description="When off, retrieval skips the vector arm regardless of whether an embedding model is configured. An effective vector recall additionally requires a model to be picked below."
            valuePropName="checked"
          >
            <Switch aria-label="Enable semantic search retrieval" data-testid="memory-semantic-enabled-switch" />
          </FormField>

          <FormField
            name="embedding_model_id"
            label="Embedding model"
            description={`The model used to compute vectors for retrieval and extraction. Switching dimension triggers a re-embed of all stored memories. Current vector dimension: ${settings.embedding_dimensions}`}
          >
            <Combobox
              data-testid="memory-semantic-model-combobox"
              placeholder={
                noModelsAvailable
                  ? 'No embedding-capable models'
                  : 'Select an embedding model'
              }
              searchPlaceholder="Search models"
              emptyText="No models found"
              loading={loadingModels}
              disabled={noModelsAvailable}
              options={embeddingModels.map((m) => ({
                value: m.id,
                label: m.display_name || m.name,
              }))}
              className="max-w-[480px]"
            />
          </FormField>

          <FormField
            name="cosine_threshold"
            label="Cosine distance threshold"
            description="Memories with distance ≥ this value are filtered out of the vector arm. Lower = stricter (fewer false-positives, more misses)."
          >
            <InputNumber min={0} max={2} step={0.05} className="w-[160px]" data-testid="memory-semantic-cosine-input" />
          </FormField>

          <div className="flex flex-col gap-1">
            <span className="text-sm font-medium">Force re-embed all memories</span>
            <Button
              icon={<RotateCw />}
              variant="outline"
              onClick={() => setReembedConfirmOpen(true)}
              disabled={!settings.embedding_model_id || !canManage}
              data-testid="memory-semantic-reembed-btn"
            >
              Re-embed now
            </Button>
            <span className="text-xs text-muted-foreground">
              Useful after an embedder upgrade or to recover from a stale
              embedding_model column. Re-embeds rows where the recorded model
              name no longer matches the current configuration.
            </span>
          </div>

        </Form>
      </Card>

      <Dialog
        data-testid="memory-semantic-reembed-dialog"
        open={reembedConfirmOpen}
        onOpenChange={(o) => {
          if (!o) setReembedConfirmOpen(false)
        }}
        title="Re-embed every memory?"
        footer={
          <Flex justify="end" className="gap-2">
            <Button variant="outline" onClick={() => setReembedConfirmOpen(false)} data-testid="memory-semantic-reembed-cancel-btn">
              Cancel
            </Button>
            <Button onClick={handleReembed} data-testid="memory-semantic-reembed-confirm-btn">Re-embed</Button>
          </Flex>
        }
      >
        <Paragraph>
          This runs in the background. Retrieval will skip rows with
          NULL embeddings (i.e., not-yet-rebuilt) and gradually catch
          up as the worker fills them in. For large memory stores this
          can take several minutes.
        </Paragraph>
      </Dialog>

      <Dialog
        data-testid="memory-semantic-swap-dialog"
        open={pendingSwap !== null}
        onOpenChange={(o) => {
          if (!o) setPendingSwap(null)
        }}
        title="Change the embedding model?"
        footer={
          <Flex justify="end" className="gap-2">
            <Button variant="outline" onClick={() => setPendingSwap(null)} data-testid="memory-semantic-swap-cancel-btn">
              Keep current model
            </Button>
            <Button
              data-testid="memory-semantic-swap-confirm-btn"
              onClick={async () => {
                if (!pendingSwap) return
                const captured = pendingSwap
                setPendingSwap(null)
                await persist(captured, true)
              }}
            >
              Change and re-embed
            </Button>
          </Flex>
        }
      >
        <Paragraph>
          Switching to <code>{swapTargetLabel}</code> will NULL every
          stored memory's embedding and re-compute it in the background.
        </Paragraph>
        <Paragraph type="secondary" className="!mb-0 text-sm">
          During the rebuild, memory retrieval returns fewer results
          (rows without an embedding are skipped). New memories created
          during the rebuild are picked up automatically. For large
          memory stores this can take several minutes.
        </Paragraph>
      </Dialog>
    </>
  )
}
