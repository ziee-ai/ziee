import { useEffect, useRef, useState } from 'react'
import {
  Typography,
  Switch,
  Form,
  Input,
  InputNumber,
  Select,
  Alert,
  Card,
  Spin,
  Button,
  Modal,
  Progress,
  Flex,
  message,
} from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Title, Paragraph } = Typography

interface AdminFormValues {
  embedding_model_id?: string | null
  default_extraction_model_id?: string | null
  enabled: boolean
  default_top_k: number
  cosine_threshold: number
  soft_delete_grace_days: number
  daily_extraction_quota: number
  summarize_after_n_messages: number
  summarizer_keep_recent: number
  full_summary_prompt?: string
  incremental_summary_prompt?: string
}

export function MemoryAdminPage() {
  const {
    settings,
    availableModels,
    rebuildStatus,
    loading,
    saving,
    loadingModels,
  } = Stores.MemoryAdmin
  const [form] = Form.useForm<AdminFormValues>()
  const [reembedConfirmOpen, setReembedConfirmOpen] = useState(false)
  // Snapshot of submitted form values held while the swap-confirm
  // Modal is open. When the admin confirms, we replay these into the
  // store; cancelling discards them.
  const [pendingSwap, setPendingSwap] = useState<AdminFormValues | null>(null)
  // Snapshot the total pending count when the rebuild started so the
  // progress bar can reflect % done instead of just remaining count.
  const rebuildTotalRef = useRef<number>(0)
  // Form goes read-only mid-session if permission is revoked. Mirrors
  // project pattern from code_sandbox/SandboxResourceLimitsSection.
  const canManage = usePermission(Permissions.MemoryAdminManage)

  useEffect(() => {
    Stores.MemoryAdmin.load()
    Stores.MemoryAdmin.loadEmbeddingCapableModels()
    Stores.MemoryAdmin.loadRebuildStatus()
  }, [])

  // Poll rebuild status while a rebuild is in flight. 2s cadence —
  // fast enough to feel responsive, slow enough that the per-row
  // worker can do real work between polls without spamming the DB.
  useEffect(() => {
    if (!rebuildStatus?.in_progress) return
    const id = setInterval(() => {
      Stores.MemoryAdmin.loadRebuildStatus()
    }, 2000)
    return () => clearInterval(id)
  }, [rebuildStatus?.in_progress])

  // Snapshot total at rebuild start so % can render meaningfully.
  useEffect(() => {
    if (
      rebuildStatus?.in_progress &&
      rebuildStatus.pending_count > rebuildTotalRef.current
    ) {
      rebuildTotalRef.current = rebuildStatus.pending_count
    }
    if (!rebuildStatus?.in_progress && rebuildStatus?.pending_count === 0) {
      rebuildTotalRef.current = 0
    }
  }, [rebuildStatus])

  // Populate form when settings arrive (or change externally via the
  // re-embed button). Matches the LlmProviderDrawer pattern. The
  // prompt fields are NULL when using the compiled-in default — we
  // surface them as empty strings in the form (placeholder shows the
  // default text), and the submit handler converts "" back to null
  // for the backend.
  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        embedding_model_id: settings.embedding_model_id,
        default_extraction_model_id: settings.default_extraction_model_id,
        enabled: settings.enabled,
        default_top_k: settings.default_top_k,
        cosine_threshold: settings.cosine_threshold,
        soft_delete_grace_days: settings.soft_delete_grace_days,
        daily_extraction_quota: settings.daily_extraction_quota,
        summarize_after_n_messages: settings.summarize_after_n_messages,
        summarizer_keep_recent: settings.summarizer_keep_recent,
        full_summary_prompt: settings.full_summary_prompt ?? '',
        incremental_summary_prompt: settings.incremental_summary_prompt ?? '',
      })
    }
  }, [settings, form])

  if (loading || !settings) {
    return (
      <div className="flex justify-center mt-8">
        <Spin />
      </div>
    )
  }

  const noModelsAvailable = availableModels.length === 0

  const persist = async (values: AdminFormValues, modelChanged: boolean) => {
    try {
      await Stores.MemoryAdmin.update({
        embedding_model_id: values.embedding_model_id ?? null,
        default_extraction_model_id:
          values.default_extraction_model_id ?? null,
        enabled: values.enabled,
        default_top_k: values.default_top_k,
        cosine_threshold: values.cosine_threshold,
        soft_delete_grace_days: values.soft_delete_grace_days,
        daily_extraction_quota: values.daily_extraction_quota,
        summarize_after_n_messages: values.summarize_after_n_messages,
        summarizer_keep_recent: values.summarizer_keep_recent,
        full_summary_prompt: values.full_summary_prompt?.trim()
          ? values.full_summary_prompt
          : null,
        incremental_summary_prompt: values.incremental_summary_prompt?.trim()
          ? values.incremental_summary_prompt
          : null,
      })
      if (modelChanged) {
        message.success(
          'Settings saved. Embedding model changed — re-embed running in background.',
        )
        // Refresh status so the progress card shows up promptly.
        Stores.MemoryAdmin.loadRebuildStatus()
      } else {
        message.success('Settings saved.')
      }
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save settings.')
    }
  }

  const handleSubmit = async (values: AdminFormValues) => {
    const priorEmbeddingId = settings.embedding_model_id
    const newEmbeddingId = values.embedding_model_id ?? null
    const modelChanged = newEmbeddingId !== priorEmbeddingId

    // If the embedding model is being swapped, intercept with a
    // confirmation Modal. The auto-spawned worker NULLs all
    // embeddings + re-embeds — admins should know the cost before
    // hitting Save. Other field changes proceed silently.
    if (modelChanged && newEmbeddingId !== null && priorEmbeddingId !== null) {
      setPendingSwap(values)
      return
    }
    await persist(values, modelChanged)
  }

  const handleReembed = async () => {
    if (!settings.embedding_model_id) return
    const ok = await Stores.MemoryAdmin.triggerReembed()
    setReembedConfirmOpen(false)
    if (ok) {
      message.info(
        'Re-embed job dispatched in background. Retrieval temporarily reduced until complete.',
      )
      Stores.MemoryAdmin.loadRebuildStatus()
    } else {
      message.error('Failed to start re-embed job.')
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
    <div className="max-w-2xl mx-auto p-6">
      <Title level={3}>Memory (admin)</Title>
      <Paragraph type="secondary">
        Configure deployment-wide memory: pick the embedding model that
        powers vector retrieval, set retrieval thresholds, and turn
        memory on or off for everyone.
      </Paragraph>

      {noModelsAvailable && (
        <Alert
          type="info"
          showIcon
          title="No embedding-capable models found."
          description={
            <span>
              Add one from the LLM Providers page — either upload a GGUF
              (e.g. <code>nomic-embed-text-v1.5</code>), download it
              from HuggingFace, or register a remote API model like{' '}
              <code>text-embedding-3-small</code>. Tick the{' '}
              <strong>text_embedding</strong> capability on the model
              form. Then return here and select it below.
            </span>
          }
          className="mb-4"
        />
      )}

      {rebuildStatus?.in_progress && (
        <Card
          className="mb-4"
          title={
            <Flex align="center" gap={8}>
              <Spin size="small" />
              <span>Re-embedding memories</span>
            </Flex>
          }
        >
          <Paragraph type="secondary" className="!mb-2 text-sm">
            Running {rebuildStatus.model_name ? <code>{rebuildStatus.model_name}</code> : 'the configured embedding model'} against every stored memory.
            Retrieval may return fewer results until this finishes; new
            memories created during the rebuild are picked up automatically.
          </Paragraph>
          <Progress
            percent={
              rebuildTotalRef.current > 0
                ? Math.max(
                    0,
                    Math.min(
                      100,
                      Math.round(
                        ((rebuildTotalRef.current - rebuildStatus.pending_count) /
                          rebuildTotalRef.current) *
                          100,
                      ),
                    ),
                  )
                : undefined
            }
            status="active"
          />
          <Paragraph type="secondary" className="!mb-0 text-xs">
            {rebuildStatus.pending_count} memory
            {rebuildStatus.pending_count === 1 ? '' : 'ies'} remaining.
          </Paragraph>
        </Card>
      )}

      <Form
        name="memory-admin-form"
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
        disabled={!canManage}
      >
        <Card title="Engine" className="mb-4">
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
            extra="Triggers the same worker that fires when the embedding model is switched. Useful after an embedder upgrade or to recover from a stale embedding_model column."
          >
            <Button
              icon={<ReloadOutlined />}
              onClick={() => setReembedConfirmOpen(true)}
              disabled={!settings.embedding_model_id}
            >
              Re-embed now
            </Button>
            <Modal
              open={reembedConfirmOpen}
              title="Re-embed every memory?"
              okText="Re-embed"
              okType="primary"
              onCancel={() => setReembedConfirmOpen(false)}
              onOk={handleReembed}
            >
              This runs in the background. Retrieval will skip rows
              with NULL embeddings (i.e., not-yet-rebuilt) and
              gradually catch up as the worker fills them in. For
              large memory stores this can take several minutes.
            </Modal>
          </Form.Item>
        </Card>

        <Card title="Retrieval tuning" className="mb-4">
          <Form.Item
            name="default_top_k"
            label="Default top-K"
            extra="How many memories to inject per turn (per user can be overridden later)."
          >
            <InputNumber min={1} max={100} />
          </Form.Item>
          <Form.Item
            name="cosine_threshold"
            label="Cosine distance threshold"
            extra="Memories with distance ≥ this value are filtered out. Lower = stricter (fewer false-positives, more misses)."
          >
            <InputNumber min={0} max={2} step={0.05} />
          </Form.Item>
        </Card>

        <Card title="Retention &amp; extraction limits" className="mb-4">
          <Form.Item
            name="soft_delete_grace_days"
            label="Soft-delete grace days"
            extra="How long soft-deleted memories stick around before the nightly reaper hard-deletes them. Lower = faster GDPR/erasure compliance; higher = longer audit window for user-initiated undeletes."
          >
            <InputNumber min={1} max={365} />
          </Form.Item>
          <Form.Item
            name="daily_extraction_quota"
            label="Daily extraction quota (per user)"
            extra="Brake against extraction-spam loops. When a user hits this many extraction-sourced memories in a 24h window, further extraction is skipped silently. The hard cost gate is your LLM API spend; this is the secondary brake on row count."
          >
            <InputNumber min={1} max={10000} />
          </Form.Item>
        </Card>

        <Card title="Conversation summarizer" className="mb-4">
          <Form.Item
            name="summarize_after_n_messages"
            label="Summarize after N messages"
            extra="When a conversation branch exceeds this many user/assistant messages, the summarizer condenses the earliest ones into a single system block. Lower = sooner summarization (smaller prompts, more LLM cost); higher = longer verbatim history."
          >
            <InputNumber min={10} max={1000} />
          </Form.Item>
          <Form.Item
            name="summarizer_keep_recent"
            label="Keep recent messages verbatim"
            extra="How many of the most-recent messages stay unsummarized alongside the summary block. Must stay below the trigger above (DB-enforced)."
            dependencies={['summarize_after_n_messages']}
            rules={[
              ({ getFieldValue }) => ({
                validator(_, value) {
                  const trigger = getFieldValue('summarize_after_n_messages')
                  if (value == null || trigger == null || value < trigger) {
                    return Promise.resolve()
                  }
                  return Promise.reject(
                    new Error(
                      `Keep-recent (${value}) must be less than the trigger (${trigger}).`,
                    ),
                  )
                },
              }),
            ]}
          >
            <InputNumber min={2} max={999} />
          </Form.Item>

          <Form.Item
            name="full_summary_prompt"
            label="Full-summarize LLM prompt"
            extra={
              <>
                Prompt sent to the summarization LLM the first time a
                branch is summarized (or when the incremental anchor is
                lost). Must contain the <code>{'{transcript}'}</code>{' '}
                placeholder. Leave empty to use the built-in default.
              </>
            }
            rules={[
              {
                validator(_, value: string | undefined) {
                  const v = value?.trim() ?? ''
                  if (v === '' || v.includes('{transcript}')) {
                    return Promise.resolve()
                  }
                  return Promise.reject(
                    new Error('Must contain the {transcript} placeholder.'),
                  )
                },
              },
            ]}
          >
            <Input.TextArea
              autoSize={{ minRows: 4, maxRows: 14 }}
              placeholder="Leave empty to use the built-in default (recommended unless you have a specific reason to tune)."
            />
          </Form.Item>

          <Form.Item
            name="incremental_summary_prompt"
            label="Incremental-refresh LLM prompt"
            extra={
              <>
                Prompt for the incremental refresh path (every
                subsequent summarization fold-in). Must contain both{' '}
                <code>{'{previous_summary}'}</code> and{' '}
                <code>{'{new_transcript}'}</code> placeholders. Leave
                empty to use the built-in default.
              </>
            }
            rules={[
              {
                validator(_, value: string | undefined) {
                  const v = value?.trim() ?? ''
                  if (
                    v === '' ||
                    (v.includes('{previous_summary}') &&
                      v.includes('{new_transcript}'))
                  ) {
                    return Promise.resolve()
                  }
                  return Promise.reject(
                    new Error(
                      'Must contain both {previous_summary} and {new_transcript} placeholders.',
                    ),
                  )
                },
              },
            ]}
          >
            <Input.TextArea
              autoSize={{ minRows: 4, maxRows: 14 }}
              placeholder="Leave empty to use the built-in default."
            />
          </Form.Item>
        </Card>

        <Flex justify="end" className="mt-4">
          <Button type="primary" htmlType="submit" loading={saving}>
            Save
          </Button>
        </Flex>
      </Form>

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
    </div>
  )
}
