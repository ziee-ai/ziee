import { useEffect, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Divider,
  Flex,
  Form,
  InputNumber,
  Modal,
  Select,
  Switch,
  Typography,
  message,
} from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Paragraph } = Typography

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

interface FormValues {
  semantic_enabled: boolean
  embedding_model_id?: string | null
  cosine_threshold: number
}

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
  const { settings, availableModels, saving, loadingModels } =
    Stores.MemoryAdmin
  const [form] = Form.useForm<FormValues>()
  const [reembedConfirmOpen, setReembedConfirmOpen] = useState(false)
  const [pendingSwap, setPendingSwap] = useState<FormValues | null>(null)

  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        semantic_enabled: settings.semantic_enabled,
        embedding_model_id: settings.embedding_model_id,
        cosine_threshold: settings.cosine_threshold,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Semantic search">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view memory admin settings."
        />
      </Card>
    )
  }
  if (!settings) return null

  const noModelsAvailable = availableModels.length === 0

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
    ? (availableModels.find((m) => m.id === pendingSwap.embedding_model_id)
        ?.display_name ??
      availableModels.find((m) => m.id === pendingSwap.embedding_model_id)
        ?.name ??
      pendingSwap.embedding_model_id)
    : ''

  return (
    <>
      <Card title="Semantic search">
        {noModelsAvailable && (
          <Alert
            type="info"
            showIcon
            className="!mb-4"
            title="No embedding-capable models found."
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
          labelCol={{ xs: { span: 24 }, md: { span: 10 } }}
          wrapperCol={{ xs: { span: 24 }, md: { span: 14 } }}
          labelAlign="left"
          colon={false}
          onFinish={handleSubmit}
          disabled={!canManage}
        >
          <Form.Item
            name="semantic_enabled"
            label="Enable semantic search"
            extra="When off, retrieval skips the vector arm regardless of whether an embedding model is configured. An effective vector recall additionally requires a model to be picked below."
            valuePropName="checked"
          >
            <Switch aria-label="Enable semantic search retrieval" />
          </Form.Item>

          <Form.Item
            name="embedding_model_id"
            label="Embedding model"
            extra={`The model used to compute vectors for retrieval and extraction. Switching dimension triggers a re-embed of all stored memories. Current vector dimension: ${settings.embedding_dimensions}`}
          >
            <Select
              placeholder={
                noModelsAvailable
                  ? 'No embedding-capable models'
                  : 'Select an embedding model'
              }
              loading={loadingModels}
              disabled={noModelsAvailable}
              options={availableModels.map((m) => ({
                value: m.id,
                label: m.display_name || m.name,
              }))}
              showSearch={{ optionFilterProp: 'label' }}
              allowClear
              style={{ maxWidth: 480 }}
            />
          </Form.Item>

          <Form.Item
            name="cosine_threshold"
            label="Cosine distance threshold"
            extra="Memories with distance ≥ this value are filtered out of the vector arm. Lower = stricter (fewer false-positives, more misses)."
          >
            <InputNumber min={0} max={2} step={0.05} style={{ width: 160 }} />
          </Form.Item>

          <Form.Item
            label="Force re-embed all memories"
            extra="Useful after an embedder upgrade or to recover from a stale embedding_model column. Re-embeds rows where the recorded model name no longer matches the current configuration."
          >
            <Button
              icon={<ReloadOutlined />}
              onClick={() => setReembedConfirmOpen(true)}
              disabled={!settings.embedding_model_id || !canManage}
            >
              Re-embed now
            </Button>
          </Form.Item>

          {canManage && (
            <>
              <Divider className="!my-3" />
              <Flex justify="end">
                <Button type="primary" htmlType="submit" loading={saving}>
                  Save
                </Button>
              </Flex>
            </>
          )}
        </Form>
      </Card>

      <Modal
        open={reembedConfirmOpen}
        title="Re-embed every memory?"
        okText="Re-embed"
        okType="primary"
        onCancel={() => setReembedConfirmOpen(false)}
        onOk={handleReembed}
      >
        <Paragraph>
          This runs in the background. Retrieval will skip rows with
          NULL embeddings (i.e., not-yet-rebuilt) and gradually catch
          up as the worker fills them in. For large memory stores this
          can take several minutes.
        </Paragraph>
      </Modal>

      <Modal
        open={pendingSwap !== null}
        title="Change the embedding model?"
        okText="Change and re-embed"
        okType="primary"
        cancelText="Keep current model"
        onCancel={() => setPendingSwap(null)}
        onOk={async () => {
          if (!pendingSwap) return
          const captured = pendingSwap
          setPendingSwap(null)
          await persist(captured, true)
        }}
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
      </Modal>
    </>
  )
}
