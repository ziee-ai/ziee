import { useEffect, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Flex,
  Form,
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
  embedding_model_id?: string | null
  default_extraction_model_id?: string | null
  enabled: boolean
}

/**
 * Embedding engine + extraction model + global enable toggle, plus the
 * explicit "Re-embed now" button. Owns its own form so saves here don't
 * re-PUT unrelated fields from other admin sections.
 */
export function EmbeddingEngineSection() {
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
        embedding_model_id: settings.embedding_model_id,
        default_extraction_model_id: settings.default_extraction_model_id,
        enabled: settings.enabled,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Engine">
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
        embedding_model_id: values.embedding_model_id ?? null,
        default_extraction_model_id:
          values.default_extraction_model_id ?? null,
        enabled: values.enabled,
      })
      if (modelChanged) {
        message.success(
          'Engine saved. Embedding model changed — re-embed running in background.',
        )
        Stores.MemoryAdmin.loadRebuildStatus()
      } else {
        message.success('Engine saved.')
      }
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to save engine settings.',
      )
    }
  }

  const handleSubmit = async (values: FormValues) => {
    const priorEmbeddingId = settings.embedding_model_id
    const newEmbeddingId = values.embedding_model_id ?? null
    const modelChanged = newEmbeddingId !== priorEmbeddingId

    if (
      modelChanged &&
      newEmbeddingId !== null &&
      priorEmbeddingId !== null
    ) {
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
    ? availableModels.find((m) => m.id === pendingSwap.embedding_model_id)
        ?.display_name ??
      availableModels.find((m) => m.id === pendingSwap.embedding_model_id)
        ?.name ??
      pendingSwap.embedding_model_id
    : ''

  return (
    <>
      <Card title="Engine">
        {noModelsAvailable && (
          <Alert
            type="info"
            showIcon
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
            className="mb-4"
          />
        )}

        <Form
          name="memory-admin-engine-form"
          form={form}
          layout="vertical"
          onFinish={handleSubmit}
          disabled={!canManage}
        >
          <Form.Item
            name="embedding_model_id"
            label="Embedding model"
            extra={
              <>
                The model used to compute vectors for both retrieval and
                extraction. Switching dimension triggers a re-embed of
                all stored memories.
                <br />
                Current vector dimension: {settings.embedding_dimensions}
              </>
            }
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
            />
          </Form.Item>

          <Form.Item
            name="default_extraction_model_id"
            label="Default extraction model"
            extra="LLM used by the silent extraction pipeline. Users can override per-account. Cheap models (Haiku-class, Gemini Flash) are ideal here."
          >
            <Select
              placeholder="Select an extraction model (optional)"
              options={availableModels.map((m) => ({
                value: m.id,
                label: m.display_name || m.name,
              }))}
              showSearch={{ optionFilterProp: 'label' }}
              allowClear
            />
          </Form.Item>

          <Form.Item
            name="enabled"
            label="Enable memory deployment-wide"
            extra="When off, all memory hooks no-op silently. Per-user toggles are unaffected but have no effect until this is on."
            valuePropName="checked"
          >
            <Switch />
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
            <Flex justify="end">
              <Button type="primary" htmlType="submit" loading={saving}>
                Save
              </Button>
            </Flex>
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
