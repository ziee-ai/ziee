import { useEffect } from 'react'
import {
  Typography,
  Switch,
  Form,
  InputNumber,
  Select,
  Alert,
  Card,
  Spin,
  message,
} from 'antd'
import { Stores } from '@/core/stores'

const { Title, Paragraph } = Typography

export function MemoryAdminPage() {
  const {
    settings,
    availableModels,
    loading,
    saving,
    loadingModels,
  } = Stores.MemoryAdmin

  useEffect(() => {
    Stores.MemoryAdmin.load()
    Stores.MemoryAdmin.loadEmbeddingCapableModels()
  }, [])

  if (loading || !settings) {
    return (
      <div className="flex justify-center mt-8">
        <Spin />
      </div>
    )
  }

  const noModelsAvailable = availableModels.length === 0

  return (
    <div className="max-w-2xl mx-auto p-6">
        <Title level={3}>Memory (admin)</Title>
        <Paragraph type="secondary">
          Configure deployment-wide memory: pick the embedding model
          that powers vector retrieval, set retrieval thresholds, and
          turn memory on or off for everyone.
        </Paragraph>

        {noModelsAvailable && (
          <Alert
            type="info"
            showIcon
            message="No embedding-capable models found."
            description={
              <span>
                Add one from the LLM Providers page — either upload a
                GGUF (e.g. <code>nomic-embed-text-v1.5</code>), download
                it from HuggingFace, or register a remote API model
                like <code>text-embedding-3-small</code>. Tick the{' '}
                <strong>text_embedding</strong> capability on the model
                form. Then return here and select it below.
              </span>
            }
            className="mb-4"
          />
        )}

        <Card title="Engine" className="mb-4">
          <Form layout="vertical">
            <Form.Item
              label="Embedding model"
              extra="The model used to compute vectors for both retrieval and extraction. Switching dimension triggers a re-embed of all stored memories."
            >
              <Select
                placeholder={
                  noModelsAvailable ? 'No embedding-capable models' : 'Select an embedding model'
                }
                value={settings.embedding_model_id ?? undefined}
                loading={loadingModels}
                disabled={noModelsAvailable}
                onChange={async (v) => {
                  await Stores.MemoryAdmin.update({ embedding_model_id: v ?? null })
                  message.success('Embedding model updated. Re-embed running in background.')
                }}
                options={availableModels.map((m) => ({
                  value: m.id,
                  label: m.display_name || m.name,
                }))}
                showSearch
                optionFilterProp="label"
                allowClear
              />
              <Paragraph type="secondary" className="!mt-1 !mb-0 text-xs">
                Current vector dimension: {settings.embedding_dimensions}
              </Paragraph>
            </Form.Item>

            <Form.Item label="Enable memory deployment-wide" extra="When off, all memory hooks no-op silently. Per-user toggles are unaffected but have no effect until this is on.">
              <Switch
                checked={settings.enabled}
                loading={saving}
                onChange={async (v) => {
                  await Stores.MemoryAdmin.update({ enabled: v })
                }}
              />
            </Form.Item>
          </Form>
        </Card>

        <Card title="Retrieval tuning" className="mb-4">
          <Form layout="vertical">
            <Form.Item
              label="Default top-K"
              extra="How many memories to inject per turn (per user can be overridden later)."
            >
              <InputNumber
                min={1}
                max={100}
                value={settings.default_top_k}
                onChange={async (v) => {
                  if (v != null) {
                    await Stores.MemoryAdmin.update({ default_top_k: v })
                  }
                }}
              />
            </Form.Item>
            <Form.Item
              label="Cosine distance threshold"
              extra="Memories with distance ≥ this value are filtered out. Lower = stricter (fewer false-positives, more misses)."
            >
              <InputNumber
                min={0}
                max={2}
                step={0.05}
                value={settings.cosine_threshold}
                onChange={async (v) => {
                  if (v != null) {
                    await Stores.MemoryAdmin.update({ cosine_threshold: v })
                  }
                }}
              />
            </Form.Item>
          </Form>
        </Card>
      </div>
  )
}
