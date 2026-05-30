import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Divider,
  Flex,
  Form,
  InputNumber,
  Spin,
  Switch,
  Typography,
  message,
} from 'antd'
// Note: previously imported Title from Typography for in-card headers.
// Switched to <Text strong> to match peer settings modules (mcp,
// assistant, llm-provider) — Title.level={4} is reserved for page
// titles via SettingsPageContainer.
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Text } = Typography

/**
 * Runtime config card (P1.j): the singleton llm_runtime_settings row.
 * idle_unload_secs / auto_start_timeout_secs / drain_timeout_secs /
 * allow_unsigned_downloads. Mirrors the peer settings module layout
 * (Text strong section header + secondary description + Form.Item;
 * Save in a justify-end flex after a Divider). Form layout="vertical"
 * already handles row spacing — no inline marginTop/marginBottom on
 * Form.Item.
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
        allow_unsigned_downloads: settings.allow_unsigned_downloads,
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
      <Form form={form} layout="vertical" disabled={!canManage}>
        <Flex vertical gap="middle">
          <Form.Item
            label={<Text strong>Idle unload timeout (seconds)</Text>}
            name="idle_unload_secs"
            help="Engines idle longer than this are automatically unloaded to free memory. 0 disables idle eviction."
            rules={[{ required: true, type: 'number', min: 0, max: 86400 }]}
          >
            <InputNumber min={0} max={86400} className="!w-[200px]" />
          </Form.Item>

          <Form.Item
            label={<Text strong>Auto-start timeout (seconds)</Text>}
            name="auto_start_timeout_secs"
            help="How long the proxy waits for a freshly-spawned engine to become healthy before giving up."
            rules={[{ required: true, type: 'number', min: 1, max: 600 }]}
          >
            <InputNumber min={1} max={600} className="!w-[200px]" />
          </Form.Item>

          <Form.Item
            label={<Text strong>Drain timeout (seconds)</Text>}
            name="drain_timeout_secs"
            help="When unloading an idle engine, how long to wait for in-flight requests to finish before forcing the stop."
            rules={[{ required: true, type: 'number', min: 1, max: 600 }]}
          >
            <InputNumber min={1} max={600} className="!w-[200px]" />
          </Form.Item>

          <Form.Item
            label={<Text strong>Allow unsigned downloads</Text>}
            name="allow_unsigned_downloads"
            valuePropName="checked"
            help="When off (default), engine binary downloads are refused because signature verification is not yet available — pre-stage binaries instead (see the pre-stage runbook). Turn on to accept unverified downloads from the upstream release pipeline during the bootstrap period."
          >
            <Switch />
          </Form.Item>
          <Form.Item dependencies={['allow_unsigned_downloads']} noStyle>
            {({ getFieldValue }) =>
              getFieldValue('allow_unsigned_downloads') ? (
                <Alert
                  type="warning"
                  showIcon
                  title="Signed-download verification disabled"
                  description="Local LLM engine downloads are not cryptographically verified. Only keep this on if you understand the supply-chain risk."
                />
              ) : null
            }
          </Form.Item>
        </Flex>

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
