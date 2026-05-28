import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Divider,
  Flex,
  Form,
  InputNumber,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

interface FormValues {
  soft_delete_grace_days: number
  daily_extraction_quota: number
}

/**
 * Retention + extraction quota. Own form.
 */
export function RetentionLimitsSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving } = Stores.MemoryAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        soft_delete_grace_days: settings.soft_delete_grace_days,
        daily_extraction_quota: settings.daily_extraction_quota,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Retention & extraction limits">
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
        soft_delete_grace_days: values.soft_delete_grace_days,
        daily_extraction_quota: values.daily_extraction_quota,
      })
      message.success('Retention & limits saved.')
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to save retention settings.',
      )
    }
  }

  return (
    <Card title="Retention &amp; extraction limits">
      <Form
        name="memory-admin-retention-form"
        form={form}
        layout="horizontal"
        labelCol={{ flex: '280px' }}
        wrapperCol={{ flex: 'auto' }}
        labelAlign="left"
        colon={false}
        onFinish={handleSubmit}
        disabled={!canManage}
      >
        <Form.Item
          name="soft_delete_grace_days"
          label="Soft-delete grace days"
          extra="How long soft-deleted memories stick around before the nightly reaper hard-deletes them. Lower = faster GDPR/erasure compliance; higher = longer audit window for user-initiated undeletes."
        >
          <InputNumber min={1} max={365} style={{ width: 160 }} />
        </Form.Item>
        <Form.Item
          name="daily_extraction_quota"
          label="Daily extraction quota (per user)"
          extra="Brake against extraction-spam loops. When a user hits this many extraction-sourced memories in a 24h window, further extraction is skipped silently. The hard cost gate is your LLM API spend; this is the secondary brake on row count."
        >
          <InputNumber min={1} max={10000} style={{ width: 160 }} />
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
