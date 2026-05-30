import { useEffect } from 'react'
import {
  Button,
  Card,
  Divider,
  Flex,
  Form,
  InputNumber,
  Spin,
  Typography,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Text } = Typography

/**
 * Runtime config card: the singleton llm_runtime_settings row —
 * idle_unload_secs / auto_start_timeout_secs / drain_timeout_secs.
 * Mirrors the peer settings module layout (Text strong section header
 * + secondary description + Form.Item; Save in a justify-end flex
 * after a Divider).
 */
export function RuntimeConfigCard() {
  const { settings, loadingSettings, savingSettings, error } =
    Stores.RuntimeConfig
  const canManage = usePermission(Permissions.RuntimeSettingsManage)
  const [form] = Form.useForm()

  useEffect(() => {
    if (settings) {
      form.setFieldsValue({
        idle_unload_secs: settings.idle_unload_secs,
        auto_start_timeout_secs: settings.auto_start_timeout_secs,
        drain_timeout_secs: settings.drain_timeout_secs,
      })
    }
  }, [settings, form])

  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.RuntimeConfig.clearError()
    }
  }, [error])

  const handleSave = async () => {
    try {
      const values = await form.validateFields()
      await Stores.RuntimeConfig.saveSettings(values)
      message.success('Runtime settings saved')
    } catch {
      // validation / save error already surfaced via the error effect
    }
  }

  if (loadingSettings && !settings) {
    return (
      <Card title="Runtime configuration">
        <Spin />
      </Card>
    )
  }

  return (
    <Card title="Runtime configuration">
      <Form
        form={form}
        layout="horizontal"
        disabled={!canManage}
        // Two columns: label on the left, input + help text on the
        // right. xs (mobile) collapses to stacked (label on top of
        // input) so neither side gets squeezed below a usable width.
        labelCol={{ xs: { span: 24 }, md: { span: 10 } }}
        wrapperCol={{ xs: { span: 24 }, md: { span: 14 } }}
        labelAlign="left"
      >
        <Form.Item
          label={<Text strong>Idle unload timeout (seconds)</Text>}
          name="idle_unload_secs"
          help="Engines idle longer than this are automatically unloaded to free memory. 0 disables idle eviction."
          rules={[{ required: true, type: 'number', min: 0, max: 86400 }]}
        >
          <InputNumber min={0} max={86400} className="!w-full" />
        </Form.Item>

        <Form.Item
          label={<Text strong>Auto-start timeout (seconds)</Text>}
          name="auto_start_timeout_secs"
          help="How long the proxy waits for a freshly-spawned engine to become healthy before giving up."
          rules={[{ required: true, type: 'number', min: 1, max: 600 }]}
        >
          <InputNumber min={1} max={600} className="!w-full" />
        </Form.Item>

        <Form.Item
          label={<Text strong>Drain timeout (seconds)</Text>}
          name="drain_timeout_secs"
          help="When unloading an idle engine, how long to wait for in-flight requests to finish before forcing the stop."
          rules={[{ required: true, type: 'number', min: 1, max: 600 }]}
        >
          <InputNumber min={1} max={600} className="!w-full" />
        </Form.Item>

        {canManage && (
          <>
            <Divider className="!my-3" />
            <Flex justify="end">
              <Button
                type="primary"
                loading={savingSettings}
                onClick={handleSave}
              >
                Save
              </Button>
            </Flex>
          </>
        )}
      </Form>
    </Card>
  )
}
