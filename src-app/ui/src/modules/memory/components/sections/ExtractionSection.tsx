import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Divider,
  Flex,
  Form,
  Select,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

interface FormValues {
  default_extraction_model_id?: string | null
}

/**
 * Memory extraction admin card: which LLM the silent extraction pipeline
 * defaults to. Per-user override is possible (Preferences); the value
 * picked here is the fallback when a user hasn't set their own.
 */
export function ExtractionSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, availableModels, saving, loadingModels } =
    Stores.MemoryAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        default_extraction_model_id: settings.default_extraction_model_id,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Extraction">
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
        default_extraction_model_id: values.default_extraction_model_id ?? null,
      })
      message.success('Extraction settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to save extraction settings.',
      )
    }
  }

  return (
    <Card title="Extraction">
      <Form
        name="memory-admin-extraction-form"
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
          name="default_extraction_model_id"
          label="Default extraction model"
          extra="LLM used by the silent extraction pipeline. Users can override per-account. Cheap models (Haiku-class, Gemini Flash) are ideal here."
        >
          <Select
            placeholder="Select an extraction model (optional)"
            loading={loadingModels}
            options={availableModels.map((m) => ({
              value: m.id,
              label: m.display_name || m.name,
            }))}
            showSearch={{ optionFilterProp: 'label' }}
            allowClear
            style={{ maxWidth: 480 }}
          />
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
