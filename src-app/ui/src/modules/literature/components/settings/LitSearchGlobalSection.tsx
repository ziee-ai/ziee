import { useEffect, useState } from 'react'
import { Alert, Button, Card, Divider, Flex, Form, InputNumber, Spin, Switch, Typography, message } from 'antd'
import { Permissions, type UpdateLitSearchSettingsRequest } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

interface CapsForm {
  max_results: number
  per_source_limit: number
  request_timeout_secs: number
}

/**
 * General card for the Literature Search settings page: master enable +
 * completeness toggle + result caps. Split out as its own section file to
 * mirror the web_search peer (WebSearchGlobalSection), keeping the page shell
 * thin.
 */
export function LitSearchGlobalSection() {
  const { settings, loading, savingSettings } = Stores.LitSearchAdmin
  const canManage = usePermission(Permissions.LitSearchAdminManage)
  const [form] = Form.useForm<CapsForm>()
  const [dirty, setDirty] = useState(false)

  useEffect(() => {
    if (settings && !dirty) {
      form.setFieldsValue({
        max_results: settings.max_results,
        per_source_limit: settings.per_source_limit,
        request_timeout_secs: settings.request_timeout_secs,
      })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings?.max_results, settings?.per_source_limit, settings?.request_timeout_secs, form, dirty])

  if (loading && !settings) {
    return (
      <Card title="General">
        <Spin />
      </Card>
    )
  }
  if (!settings) return null

  const save = async (patch: UpdateLitSearchSettingsRequest, label = 'Saved') => {
    try {
      await Stores.LitSearchAdmin.updateSettings(patch)
      message.success(label)
      setDirty(false)
    } catch (e: any) {
      message.error(e?.message ?? 'Update failed')
    }
  }

  return (
    <Card title="General">
      {!canManage && (
        <Alert
          type="info"
          showIcon
          title="Read-only view"
          description="You can view literature search settings but not change them."
          className="mb-3"
        />
      )}
      <Flex align="center" gap="small" className="mb-3">
        <Switch
          aria-label="Enable literature search"
          checked={settings.enabled}
          disabled={!canManage}
          onChange={v => save({ enabled: v }, v ? 'Literature search enabled' : 'Disabled')}
        />
        <Typography.Text>Enable literature search</Typography.Text>
      </Flex>

      <Flex align="center" gap="small" className="mb-3">
        <Switch
          aria-label="Show completeness estimate"
          checked={settings.completeness_estimate_enabled}
          disabled={!canManage}
          onChange={v => save({ completeness_estimate_enabled: v }, 'Completeness estimate updated')}
        />
        <Typography.Text>Show completeness (saturation) estimate</Typography.Text>
      </Flex>

      <Typography.Paragraph type="secondary" className="text-xs">
        The saturation estimate is a heuristic — never a measured recall rate. This
        feature is an adjunct to, not a replacement for, systematic searching.
      </Typography.Paragraph>

      <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
        <Typography.Text className="text-sm">Caps</Typography.Text>
      </Divider>

      <Form
        form={form}
        name="lit-caps"
        layout="horizontal"
        labelCol={{ xs: { span: 24 }, md: { span: 10 } }}
        wrapperCol={{ xs: { span: 24 }, md: { span: 8 } }}
        labelAlign="left"
        colon={false}
        disabled={!canManage}
        onValuesChange={() => setDirty(true)}
        onFinish={v => save(v, 'Literature search settings saved')}
      >
        <Form.Item label="Max deduped results" name="max_results">
          <InputNumber min={1} max={200} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item label="Per-source limit" name="per_source_limit">
          <InputNumber min={1} max={100} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item label="Request timeout (s)" name="request_timeout_secs">
          <InputNumber min={1} max={120} style={{ width: '100%' }} />
        </Form.Item>
        <Flex justify="end">
          <Button type="primary" htmlType="submit" loading={savingSettings} disabled={!canManage || !dirty}>
            Save caps
          </Button>
        </Flex>
      </Form>
    </Card>
  )
}
