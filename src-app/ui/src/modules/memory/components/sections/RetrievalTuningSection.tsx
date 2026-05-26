import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
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
  default_top_k: number
  cosine_threshold: number
}

/**
 * Retrieval tuning: top-K + cosine_threshold. Own form so saves don't
 * trip the embedding-model swap path.
 */
export function RetrievalTuningSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving } = Stores.MemoryAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        default_top_k: settings.default_top_k,
        cosine_threshold: settings.cosine_threshold,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Retrieval tuning">
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
        default_top_k: values.default_top_k,
        cosine_threshold: values.cosine_threshold,
      })
      message.success('Retrieval tuning saved.')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save retrieval tuning.')
    }
  }

  return (
    <Card title="Retrieval tuning">
      <Form
        name="memory-admin-retrieval-form"
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
        disabled={!canManage}
      >
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

        {canManage && (
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
