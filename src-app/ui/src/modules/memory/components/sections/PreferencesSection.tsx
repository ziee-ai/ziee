import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Flex,
  Form,
  InputNumber,
  Space,
  Spin,
  Switch,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'


const READ_PERM = Permissions.MemoryRead
const WRITE_PERM = Permissions.MemoryWrite

interface FormValues {
  extraction_enabled: boolean
  retrieval_enabled: boolean
  max_memories: number
  retention_days: number | null
}

/**
 * Per-user memory preferences: extraction/retrieval toggles + storage caps.
 *
 * Hidden entirely if the viewer doesn't have `memory::read`. The page
 * itself is gated on the `MEMORY_USER_READ_PERM` anyOf, so a user
 * with only `memory::core::read` reaches the page — but this section
 * is skipped because the underlying preferences are owned by the
 * vector-memory subsystem the user doesn't have access to.
 */
export function PreferencesSection() {
  const canRead = usePermission(READ_PERM)
  const canWrite = usePermission(WRITE_PERM)
  const { settings, loading, saving } = Stores.MemorySettings
  const { settings: adminSettings } = Stores.MemoryAdmin
  const [form] = Form.useForm<FormValues>()

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

  if (!canRead) return null

  const adminDisabled = adminSettings && !adminSettings.enabled

  if (loading || !settings) {
    return (
      <Card title="Preferences">
        <div className="flex justify-center py-6">
          <Spin />
        </div>
      </Card>
    )
  }

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.MemorySettings.update({
        extraction_enabled: values.extraction_enabled,
        retrieval_enabled: values.retrieval_enabled,
        max_memories: values.max_memories,
        retention_days: values.retention_days ?? null,
      })
      message.success('Preferences saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save preferences.',
      )
    }
  }

  return (
    <Card title="Preferences">
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
        name="memory-preferences-form"
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
        disabled={!canWrite}
      >
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
              disabled={!canWrite}
              onClick={() => form.setFieldValue('retention_days', null)}
            >
              Forever
            </Button>
          </Space>
        </Form.Item>

        {canWrite && (
          <Flex justify="end">
            <Button type="primary" htmlType="submit" loading={saving}>
              Save
            </Button>
          </Flex>
        )}
      </Form>
    </Card>
  )
}
