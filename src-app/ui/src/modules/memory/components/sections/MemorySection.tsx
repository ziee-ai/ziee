import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Divider,
  Flex,
  Form,
  InputNumber,
  Switch,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

interface FormValues {
  enabled: boolean
  default_top_k: number
}

/**
 * Master memory card: deployment-wide kill switch + the shared
 * `default_top_k` retrieval cap. Per-arm enable toggles
 * (`fts_enabled`, `semantic_enabled`) live in their own cards below.
 */
export function MemorySection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving } = Stores.MemoryAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        enabled: settings.enabled,
        default_top_k: settings.default_top_k,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Memory">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view memory admin settings."
        />
      </Card>
    )
  }
  if (!settings) return null

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.MemoryAdmin.update({
        enabled: values.enabled,
        default_top_k: values.default_top_k,
      })
      message.success('Memory settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save memory settings.',
      )
    }
  }

  return (
    <Card title="Memory">
      <Form
        name="memory-admin-master-form"
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
          name="enabled"
          label="Enable memory deployment-wide"
          extra="When off, all memory hooks no-op silently. Per-user toggles are unaffected but have no effect until this is on."
          valuePropName="checked"
        >
          <Switch aria-label="Enable memory deployment-wide" />
        </Form.Item>

        <Form.Item
          name="default_top_k"
          label="Default top-K"
          extra="How many memories to inject per turn. Shared across retrieval arms — the fused top-K is what's injected, whether the result came from full-text, semantic, or hybrid search. Users can override their own limit later."
        >
          <InputNumber min={1} max={100} style={{ width: 160 }} />
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
  )
}
