import { useEffect } from 'react'
import {
  Typography,
  Switch,
  Form,
  InputNumber,
  Alert,
  Card,
  Spin,
  Space,
  Button,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Title, Paragraph } = Typography

export function MemorySettingsPage() {
  const { settings, loading, saving } = Stores.MemorySettings
  const { settings: adminSettings } = Stores.MemoryAdmin
  // Per-user toggles read-only mid-session if permission is revoked.
  const canWrite = usePermission(Permissions.MemoryWrite)

  useEffect(() => {
    Stores.MemorySettings.load()
    Stores.MemoryAdmin.load().catch(() => {
      // Non-admins can't read admin settings; the banner just won't render.
    })
  }, [])

  if (loading || !settings) {
    return (
      <div className="flex justify-center mt-8">
        <Spin />
      </div>
    )
  }

  const adminDisabled = adminSettings && !adminSettings.enabled

  return (
    <div className="max-w-2xl mx-auto p-6">
        <Title level={3}>Memory</Title>
        <Paragraph type="secondary">
          Persistent memory lets the assistant remember facts about you
          across conversations. Two independent switches: auto-extraction
          captures new facts from your chats, and retrieval surfaces
          them in future replies. Both are off by default.
        </Paragraph>

        {adminDisabled && (
          <Alert
            type="warning"
            showIcon
            message="Memory is currently disabled by the administrator."
            description="Settings here will be saved but have no effect until the administrator enables memory."
            className="mb-4"
          />
        )}

        <Card title="Toggles" className="mb-4">
          <Form layout="horizontal" labelCol={{ span: 10 }} disabled={!canWrite}>
            <Form.Item label="Auto-extract memories">
              <Switch
                checked={settings.extraction_enabled}
                loading={saving}
                onChange={async (v) => {
                  await Stores.MemorySettings.update({ extraction_enabled: v })
                }}
              />
              <Paragraph type="secondary" className="!mt-1 !mb-0 text-xs">
                After each assistant reply, an LLM scans your turn for
                durable facts about you and stores them.
              </Paragraph>
            </Form.Item>
            <Form.Item label="Inject relevant memories on retrieval">
              <Switch
                checked={settings.retrieval_enabled}
                loading={saving}
                onChange={async (v) => {
                  await Stores.MemorySettings.update({ retrieval_enabled: v })
                }}
              />
              <Paragraph type="secondary" className="!mt-1 !mb-0 text-xs">
                Before each LLM call, your latest message is embedded
                and the top-K most-similar memories are added to the
                system prompt.
              </Paragraph>
            </Form.Item>
          </Form>
        </Card>

        <Card title="Limits" className="mb-4">
          <Form layout="vertical" disabled={!canWrite}>
            <Form.Item label="Max memories stored" extra="When this cap is reached the reaper soft-deletes the oldest.">
              <InputNumber
                min={1}
                max={100000}
                value={settings.max_memories}
                onChange={async (v) => {
                  if (v != null) {
                    await Stores.MemorySettings.update({ max_memories: v })
                  }
                }}
              />
            </Form.Item>
            <Form.Item label="Retention (days)" extra="Empty = forever. Older memories are soft-deleted by the nightly reaper.">
              <Space>
                <InputNumber
                  min={1}
                  max={3650}
                  value={settings.retention_days ?? undefined}
                  onChange={async (v) => {
                    await Stores.MemorySettings.update({
                      retention_days: v != null ? v : null,
                    })
                  }}
                />
                <Button
                  size="small"
                  onClick={async () => {
                    await Stores.MemorySettings.update({ retention_days: null })
                    message.success('Retention set to forever')
                  }}
                >
                  Forever
                </Button>
              </Space>
            </Form.Item>
          </Form>
        </Card>
      </div>
  )
}
