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
  Tooltip,
  Typography,
  message,
} from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Paragraph } = Typography

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

interface FormValues {
  semantic_enabled: boolean
  embedding_model_id?: string | null
  cosine_threshold: number
}

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
  const { settings, embeddingModels, saving, loadingModels, triggeringReembed } =
    Stores.FileRagAdmin
  const [form] = Form.useForm<FormValues>()
  const [reembedConfirmOpen, setReembedConfirmOpen] = useState(false)
  const [pendingSwap, setPendingSwap] = useState<FormValues | null>(null)

  useEffect(() => {
    // Don't clobber the admin's unsaved edits on a mid-edit refetch.
    if (settings && !form.isFieldsTouched()) {
      form.setFieldsValue({
        semantic_enabled: settings.semantic_enabled,
        embedding_model_id: settings.embedding_model_id,
        cosine_threshold: settings.cosine_threshold,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Embedding (semantic search)">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings) return null

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
      <Card title="Embedding (semantic search)">
        {noModelsAvailable && (
          <Alert
            type="info"
            showIcon
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
          name="file-rag-admin-embedding-form"
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
            extra="When off, retrieval uses full-text only. Effective semantic recall also requires an embedding model below."
            valuePropName="checked"
          >
            <Switch aria-label="Enable semantic search" />
          </Form.Item>

          <Form.Item
            name="embedding_model_id"
            label="Embedding model"
            extra={`The model used to compute document + query vectors. The vector dimension is derived automatically from the model. Current dimension: ${settings.embedding_dimensions}`}
          >
            <Select
              placeholder={
                noModelsAvailable
                  ? 'No embedding-capable models'
                  : 'Select an embedding model'
              }
              loading={loadingModels}
              disabled={noModelsAvailable}
              options={embeddingModels.map(m => ({
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
            extra="Chunks with distance ≥ this value are dropped from the vector arm. Lower = stricter."
          >
            <InputNumber min={0} max={2} step={0.05} style={{ width: 160 }} />
          </Form.Item>

          <Form.Item
            label="Force re-embed all chunks"
            extra="Re-embeds every stored chunk with the current model. Useful after an embedder upgrade."
          >
            <Tooltip
              title={
                !settings.embedding_model_id
                  ? 'Select an embedding model first'
                  : !canManage
                    ? "You don't have permission to manage file RAG settings."
                    : undefined
              }
            >
              <Button
                icon={<ReloadOutlined />}
                loading={triggeringReembed}
                onClick={() => setReembedConfirmOpen(true)}
                disabled={!settings.embedding_model_id || !canManage}
              >
                Re-embed now
              </Button>
            </Tooltip>
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
        title="Re-embed every document chunk?"
        okText="Re-embed"
        okType="primary"
        onCancel={() => setReembedConfirmOpen(false)}
        onOk={handleReembed}
      >
        <Paragraph>
          This runs in the background. Semantic search skips not-yet-embedded
          chunks (full-text still works) and catches up as the worker fills them
          in. For large corpora this can take several minutes.
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
          Switching to <code>{swapTargetLabel}</code> re-computes every stored
          chunk's embedding in the background (and rebuilds the vector index if
          the dimension differs).
        </Paragraph>
        <Paragraph type="secondary" className="!mb-0 text-sm">
          During the rebuild, semantic search returns fewer results; full-text
          search is unaffected. New uploads are picked up automatically.
        </Paragraph>
      </Modal>
    </>
  )
}
