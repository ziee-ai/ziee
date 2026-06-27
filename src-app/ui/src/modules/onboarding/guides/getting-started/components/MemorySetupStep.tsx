import { useEffect, useState } from 'react'
import {
  Text,
  Title,
  Paragraph,
  Switch,
  Select,
  Alert,
  Spin,
  Tag,
  Button,
  Space,
  Flex,
} from '@/components/ui'
import {
  BulbOutlined,
  InfoCircleOutlined,
  ArrowLeftOutlined,
  ReloadOutlined,
  PlusOutlined,
} from '@ant-design/icons'
import type { OnboardingStepProps } from '@/modules/onboarding/types/onboarding'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

/**
 * MemorySetupStep — Plan §8 two-screen flow.
 *
 *   Screen 1: "Enable persistent memory?" with Yes / Skip.
 *   Screen 2 (only if Yes): "Pick an embedding model" with a dropdown
 *                           filtered to text_embedding=true models,
 *                           plus inline "Add embedding model" launcher
 *                           when none exist.
 *
 * The wizard's Next button calls registerBeforeNext which persists
 * settings via PUT /api/memory/admin-settings. Skip + Next leaves
 * memory disabled (enabled=false, embedding_model_id=NULL).
 */
export default function MemorySetupStep({ registerBeforeNext }: OnboardingStepProps) {
  const {
    enableMemory,
    embeddingModelId,
    availableModels,
    loading,
    saving,
    error,
  } = Stores.MemorySetupStep

  const [screen, setScreen] = useState<'enable' | 'pick'>('enable')

  // Only root admin / memory-admin-capable users see the controls.
  const canManageMemory = usePermission(Permissions.MemoryAdminManage)

  useEffect(() => {
    Stores.Onboarding.setReady(true)

    // Save on Next: returns void; saveSettings updates the store's
    // error state which is surfaced via the in-component Alert.
    registerBeforeNext(async () => {
      if (!canManageMemory) return
      await Stores.MemorySetupStep.saveSettings()
    })

    if (canManageMemory) {
      Stores.MemorySetupStep.loadEmbeddingCapableModels()
    }
  }, [canManageMemory])

  // Auto-advance to screen 2 when the user toggles memory on (and back
  // when they toggle off).
  useEffect(() => {
    if (enableMemory && screen === 'enable') setScreen('pick')
    if (!enableMemory && screen === 'pick') setScreen('enable')
  }, [enableMemory])

  if (!canManageMemory) {
    return (
      <div className="max-w-lg">
        <div className="flex items-center gap-3 mb-4">
          <BulbOutlined className="text-3xl text-amber-500" />
          <Title level={3} className="!mb-0">
            Persistent Memory
          </Title>
        </div>
        <Paragraph type="secondary">
          Persistent memory lets the assistant remember facts about you
          across conversations. Your administrator controls whether
          this is enabled deployment-wide. Once available, you can
          manage your individual memories from the Memory settings page.
        </Paragraph>
      </div>
    )
  }

  if (loading) {
    return (
      <div className="flex justify-center mt-8">
        <Spin label="Loading" />
      </div>
    )
  }

  // ── Screen 2: pick the embedding model ────────────────────────────
  if (screen === 'pick' && enableMemory) {
    return (
      <PickModelScreen
        embeddingModelId={embeddingModelId}
        availableModels={availableModels}
        error={error}
        saving={saving}
        onBack={() => {
          Stores.MemorySetupStep.setEnableMemory(false)
          setScreen('enable')
        }}
      />
    )
  }

  // ── Screen 1: enable choice ───────────────────────────────────────
  return (
    <div className="max-w-xl">
      <div className="flex items-center gap-3 mb-4">
        <BulbOutlined className="text-3xl text-amber-500" />
        <Title level={3} className="!mb-0">
          Persistent Memory
        </Title>
      </div>

      <Paragraph type="secondary">
        Memory lets the assistant remember facts about each user across
        conversations &mdash; preferences, goals, recurring topics
        &mdash; using a vector retrieval layer over Postgres.
        It&rsquo;s off by default for privacy.
      </Paragraph>

      {error && (
        <Alert tone="error" title={error} className="mb-4" />
      )}

      <div className="border rounded-lg p-4 mb-4">
        <div className="flex items-center justify-between">
          <div>
            <Text strong>Enable persistent memory?</Text>
            <div>
              <Text type="secondary" className="text-sm">
                Turn on memory extraction and retrieval for this deployment.
                Skip if you want to revisit later from the Memory admin page.
              </Text>
            </div>
          </div>
          <Switch
            checked={enableMemory}
            onChange={(checked) => Stores.MemorySetupStep.setEnableMemory(checked)}
          />
        </div>
      </div>

      {saving && (
        <Paragraph type="secondary" className="text-right">
          Saving&hellip;
        </Paragraph>
      )}
    </div>
  )
}

function PickModelScreen({
  embeddingModelId,
  availableModels,
  error,
  saving,
  onBack,
}: {
  embeddingModelId: string | null
  availableModels: { id: string; name: string; display_name: string | null; provider_id: string }[]
  error: string | null
  saving: boolean
  onBack: () => void
}) {
  const noModelsAvailable = availableModels.length === 0
  const [refreshing, setRefreshing] = useState(false)

  return (
    <div className="max-w-xl">
      <div className="flex items-center gap-3 mb-4">
        <Button
          icon={<ArrowLeftOutlined />}
          size="sm"
          onClick={onBack}
          aria-label="Back"
        />
        <BulbOutlined className="text-3xl text-amber-500" />
        <Title level={3} className="!mb-0">
          Pick an embedding model
        </Title>
      </div>

      <Paragraph type="secondary">
        Memory needs a model to compute vector embeddings. Local GGUF
        models (e.g. <code>nomic-embed-text-v1.5</code>) work fully
        offline; remote API models (OpenAI / Gemini) work without a
        bundled GPU. Either option is fine.
      </Paragraph>

      {error && (
        <Alert tone="error" title={error} className="mb-4" />
      )}

      <div className="mb-2 flex items-center gap-2">
        <Text strong>Embedding model</Text>
        {noModelsAvailable && (
          <Tag tone="warning">No embedding-capable models</Tag>
        )}
      </div>

      {noModelsAvailable ? (
        <Alert
          tone="info"
          icon={<InfoCircleOutlined />}
          title="No embedding-capable models found."
          description={
            <Flex vertical className="w-full gap-2">
              <Text>
                Add one from the LLM Providers page. The Hub catalog
                ships curated entries (<code>nomic-embed-text-v1.5</code>,
                <code>bge-small-en-v1.5</code>,{' '}
                <code>mxbai-embed-large-v1</code>) one-click installable;
                or upload a GGUF; or register a remote API model. Tick the
                {' '}<strong>text_embedding</strong>{' '}capability before
                saving.
              </Text>
              <Space>
                <Button
                  variant="default"
                  icon={<PlusOutlined />}
                  onClick={() => {
                    // Open the LLM Providers page in a NEW tab so the
                    // wizard state is preserved. The admin adds the
                    // model, comes back, hits Refresh.
                    window.open('/llm-providers', '_blank', 'noopener')
                  }}
                >
                  Add embedding model
                </Button>
                <Button
                  icon={<ReloadOutlined />}
                  loading={refreshing}
                  onClick={async () => {
                    setRefreshing(true)
                    try {
                      await Stores.MemorySetupStep.loadEmbeddingCapableModels()
                    } finally {
                      setRefreshing(false)
                    }
                  }}
                >
                  Refresh
                </Button>
              </Space>
            </Flex>
          }
          className="mb-4"
        />
      ) : (
        <Select
          className="w-full mb-4"
          placeholder="Select an embedding model"
          value={embeddingModelId ?? undefined}
          onChange={(v) => Stores.MemorySetupStep.setEmbeddingModelId(v ?? null)}
          options={availableModels.map((m) => ({
            value: m.id,
            label: m.display_name || m.name,
          }))}
        />
      )}

      {saving && (
        <Paragraph type="secondary" className="text-right">
          Saving&hellip;
        </Paragraph>
      )}
    </div>
  )
}
