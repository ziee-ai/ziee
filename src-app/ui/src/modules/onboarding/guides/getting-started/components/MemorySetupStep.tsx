import { useEffect } from 'react'
import {
  Typography,
  Switch,
  Select,
  Alert,
  Spin,
  Tag,
} from 'antd'
import { BulbOutlined, InfoCircleOutlined } from '@ant-design/icons'
import type { OnboardingStepProps } from '@/modules/onboarding/types/onboarding'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Title, Paragraph, Text } = Typography

export default function MemorySetupStep({ registerBeforeNext }: OnboardingStepProps) {
  const {
    enableMemory,
    embeddingModelId,
    availableModels,
    loading,
    saving,
    error,
  } = Stores.MemorySetupStep

  // Only root admin / memory-admin-capable users see the controls.
  // Non-admins get a brief intro and continue.
  const canManageMemory = usePermission(Permissions.MemoryAdminManage)

  useEffect(() => {
    Stores.Onboarding.setReady(true)

    // Save on Next: if memory is being enabled, the save call validates
    // a model is picked. Errors are surfaced via the in-component
    // Alert (set on the store) so the user can act before clicking
    // Next again.
    registerBeforeNext(async () => {
      if (!canManageMemory) {
        return
      }
      await Stores.MemorySetupStep.saveSettings()
    })

    if (canManageMemory) {
      Stores.MemorySetupStep.loadEmbeddingCapableModels()
    }
  }, [canManageMemory])

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
        <Spin />
      </div>
    )
  }

  const noModelsAvailable = availableModels.length === 0

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
        conversations — preferences, goals, recurring topics — using a
        vector retrieval layer over Postgres. It&rsquo;s off by default
        for privacy. You can enable it deployment-wide here, or skip
        and revisit later from the Memory admin page.
      </Paragraph>

      {error && (
        <Alert type="error" message={error} showIcon className="mb-4" />
      )}

      <div className="border rounded-lg p-4 mb-4">
        <div className="flex items-center justify-between">
          <div>
            <Text strong>Enable persistent memory</Text>
            <div>
              <Text type="secondary" className="text-sm">
                Turn on memory extraction and retrieval for this deployment.
              </Text>
            </div>
          </div>
          <Switch
            checked={enableMemory}
            onChange={(checked) => Stores.MemorySetupStep.setEnableMemory(checked)}
          />
        </div>
      </div>

      {enableMemory && (
        <>
          <div className="mb-2 flex items-center gap-2">
            <Text strong>Embedding model</Text>
            {noModelsAvailable && (
              <Tag color="orange">No embedding-capable models</Tag>
            )}
          </div>

          {noModelsAvailable ? (
            <Alert
              type="info"
              showIcon
              icon={<InfoCircleOutlined />}
              message="No embedding-capable models found."
              description={
                <div>
                  <div>
                    Add a model with the <code>text_embedding</code>{' '}
                    capability from the LLM Providers page — either
                    upload a GGUF, download from HuggingFace
                    (e.g. <code>nomic-embed-text-v1.5</code>), or add a
                    remote API model.
                  </div>
                  <div className="mt-2">
                    You can return to this step from the Memory admin
                    page after adding one.
                  </div>
                </div>
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
              showSearch
              optionFilterProp="label"
            />
          )}
        </>
      )}

      {/* No in-page Save button — registerBeforeNext fires on the
          wizard's Next click and persists the toggle + model id.
          Avoids gap #30: double-save when both buttons exist. */}
      {saving && (
        <Paragraph type="secondary" className="text-right">
          Saving…
        </Paragraph>
      )}
    </div>
  )
}
