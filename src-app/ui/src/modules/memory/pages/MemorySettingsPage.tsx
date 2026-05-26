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
  Flex,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Title, Paragraph } = Typography

interface SettingsFormValues {
  extraction_enabled: boolean
  retrieval_enabled: boolean
  max_memories: number
  retention_days: number | null
}

export function MemorySettingsPage() {
  const { settings, loading, saving } = Stores.MemorySettings
  const { settings: adminSettings } = Stores.MemoryAdmin
  const [form] = Form.useForm<SettingsFormValues>()
  // Per-user toggles read-only mid-session if permission is revoked.
  const canWrite = usePermission(Permissions.MemoryWrite)

  useEffect(() => {
    Stores.MemorySettings.load()
    Stores.MemoryAdmin.load().catch(() => {
      // Non-admins can't read admin settings; the banner just won't render.
    })
  }, [])

  // Populate form when settings arrive.
  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        extraction_enabled: settings.extraction_enabled,
        retrieval_enabled: settings.retrieval_enabled,
        max_memories: settings.max_memories,
        retention_days: settings.retention_days,
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

  const adminDisabled = adminSettings && !adminSettings.enabled

  const handleSubmit = async (values: SettingsFormValues) => {
    try {
      await Stores.MemorySettings.update({
        extraction_enabled: values.extraction_enabled,
        retrieval_enabled: values.retrieval_enabled,
        max_memories: values.max_memories,
        // Empty number input arrives as null/undefined → clear to NULL.
        retention_days: values.retention_days ?? null,
      })
      message.success('Memory settings saved.')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save settings.')
    }
  }

  return (
    <div className="max-w-2xl mx-auto p-6">
      <Title level={3}>Memory</Title>
      <Paragraph type="secondary">
        Persistent memory lets the assistant remember facts about you
        across conversations. Two independent switches: auto-extraction
        captures new facts from your chats, and retrieval surfaces them
        in future replies. Both are off by default.
      </Paragraph>

      {adminDisabled && (
        <Alert
          type="warning"
          showIcon
          title="Memory is currently disabled by the administrator."
          description="Settings here will be saved but have no effect until the administrator enables memory."
          className="mb-4"
        />
      )}

      <Form
        name="memory-settings-form"
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
        disabled={!canWrite}
      >
        <Card title="Toggles" className="mb-4">
          <Form.Item
            name="extraction_enabled"
            label="Auto-extract memories"
            valuePropName="checked"
            extra="After each assistant reply, an LLM scans your turn for durable facts about you and stores them."
          >
            <Switch />
          </Form.Item>
          <Form.Item
            name="retrieval_enabled"
            label="Inject relevant memories on retrieval"
            valuePropName="checked"
            extra="Before each LLM call, your latest message is embedded and the top-K most-similar memories are added to the system prompt."
          >
            <Switch />
          </Form.Item>
        </Card>

        <Card title="Limits" className="mb-4">
          <Form.Item
            name="max_memories"
            label="Max memories stored"
            extra="When this cap is reached the reaper soft-deletes the oldest."
          >
            <InputNumber min={1} max={100000} />
          </Form.Item>
          <Form.Item
            label="Retention (days)"
            extra="Empty = forever. Older memories are soft-deleted by the nightly reaper."
          >
            <Space>
              <Form.Item name="retention_days" noStyle>
                <InputNumber min={1} max={3650} />
              </Form.Item>
              <Button
                size="small"
                onClick={() => form.setFieldValue('retention_days', null)}
              >
                Forever
              </Button>
            </Space>
          </Form.Item>
        </Card>

        <Flex justify="end" className="mt-4">
          <Button type="primary" htmlType="submit" loading={saving}>
            Save
          </Button>
        </Flex>
      </Form>
    </div>
  )
}
