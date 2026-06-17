import { useEffect, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Divider,
  Flex,
  Form,
  InputNumber,
  List,
  Select,
  Space,
  Spin,
  Switch,
  Tooltip,
  Typography,
  message,
} from 'antd'
import { ArrowDownOutlined, ArrowUpOutlined, DeleteOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const MIB = 1024 * 1024

type FormValues = {
  enabled: boolean
  max_results: number
  fetch_max_mib: number
  fetch_max_chars: number
  request_timeout_secs: number
}

/**
 * Global web-search settings: the master enable switch, the ordered
 * provider fallback chain, and the request caps. The chain editor saves on
 * each reorder/add/remove; the caps form saves on its own Save button.
 */
export function WebSearchGlobalSection() {
  const { settings, providers, loading, savingSettings } = Stores.WebSearchAdmin
  const canManage = usePermission(Permissions.WebSearchAdminManage)

  const [form] = Form.useForm<FormValues>()
  const [dirty, setDirty] = useState(false)
  // Local in-flight flag for chain edits, so they don't share the store's
  // `savingSettings` flag with the caps Save button (which would cross-trigger
  // the caps spinner on a chain edit and vice-versa).
  const [savingChain, setSavingChain] = useState(false)

  // Re-seed from the store ONLY when the form has no unsaved edits. The chain
  // editor saves imperatively (move/add/remove → updateSettings), which
  // replaces `settings`; without the `!dirty` guard that re-seed would clobber
  // in-progress caps/enabled edits the admin hasn't saved yet.
  useEffect(() => {
    if (settings && !dirty) {
      form.setFieldsValue({
        enabled: settings.enabled,
        max_results: settings.max_results,
        fetch_max_mib: Math.round(settings.fetch_max_bytes / MIB),
        fetch_max_chars: settings.fetch_max_chars,
        request_timeout_secs: settings.request_timeout_secs,
      })
    }
  }, [settings, form, dirty])

  const onSubmit = async (v: FormValues) => {
    try {
      await Stores.WebSearchAdmin.updateSettings({
        enabled: v.enabled,
        max_results: v.max_results,
        fetch_max_bytes: v.fetch_max_mib * MIB,
        fetch_max_chars: v.fetch_max_chars,
        request_timeout_secs: v.request_timeout_secs,
      })
      setDirty(false) // saved → allow the next store update to re-seed
      message.success('Web search settings saved')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save')
    }
  }

  const chain = settings?.provider_chain ?? []
  const nameOf = (key: string) =>
    providers.find(p => p.key === key)?.display_name ?? key
  const configuredOf = (key: string) =>
    providers.find(p => p.key === key)?.configured ?? false
  const notInChain = providers.filter(p => !chain.includes(p.key))

  const saveChain = async (next: string[]) => {
    setSavingChain(true)
    try {
      await Stores.WebSearchAdmin.updateSettings({ provider_chain: next })
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to update provider chain')
    } finally {
      setSavingChain(false)
    }
  }

  const move = (i: number, dir: -1 | 1) => {
    const next = [...chain]
    const j = i + dir
    if (j < 0 || j >= next.length) return
    ;[next[i], next[j]] = [next[j], next[i]]
    void saveChain(next)
  }
  const remove = (i: number) => void saveChain(chain.filter((_, idx) => idx !== i))
  const add = (key: string) => void saveChain([...chain, key])

  if (loading && !settings) {
    return (
      <Card title="Web search">
        <Spin />
      </Card>
    )
  }

  return (
    <Card title="Web search">
      {!canManage && (
        <Alert
          type="info"
          showIcon
          title="Read-only view"
          description="You can view web search settings but not change them."
          className="mb-3"
        />
      )}

      <Form
        form={form}
        layout="horizontal"
        labelCol={{ xs: { span: 24 }, md: { span: 10 } }}
        wrapperCol={{ xs: { span: 24 }, md: { span: 14 } }}
        labelAlign="left"
        colon={false}
        onFinish={onSubmit}
        onValuesChange={() => setDirty(true)}
        disabled={!canManage}
      >
        <Form.Item
          name="enabled"
          label="Enabled"
          valuePropName="checked"
          extra="Master switch. Even when on, web tools only attach to a chat once a provider in the chain is configured."
        >
          <Switch />
        </Form.Item>

        <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
          <Typography.Text type="secondary" className="text-xs">
            Caps
          </Typography.Text>
        </Divider>
        <Form.Item name="max_results" label="Max results per search">
          <InputNumber min={1} max={20} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item
          name="fetch_max_mib"
          label="Page fetch size cap"
          extra="Maximum bytes downloaded per fetch_url call."
        >
          <InputNumber min={1} max={100} suffix="MiB" style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item
          name="fetch_max_chars"
          label="Page fetch char cap"
          extra="Extracted markdown is truncated to this many characters."
        >
          <InputNumber min={1000} max={500000} step={1000} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="request_timeout_secs" label="Request timeout">
          <InputNumber min={1} max={120} suffix="s" style={{ width: '100%' }} />
        </Form.Item>

        <Flex justify="end" gap="small">
          <Button
            type="primary"
            htmlType="submit"
            loading={savingSettings}
            disabled={!canManage || !dirty}
          >
            Save
          </Button>
        </Flex>
      </Form>

      <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
        <Typography.Text type="secondary" className="text-xs">
          Provider chain
        </Typography.Text>
      </Divider>
      <Typography.Paragraph type="secondary" className="text-xs">
        Engines are tried top-to-bottom. The chain advances to the next engine
        only on failure (error / timeout / quota) — an engine returning no
        results is treated as a valid answer.
      </Typography.Paragraph>

      {chain.length === 0 ? (
        <Alert
          type="warning"
          showIcon
          title="No providers in the chain"
          description="Add at least one provider below and configure it for web search to work."
          className="mb-3"
        />
      ) : (
        <List
          size="small"
          bordered
          dataSource={chain}
          renderItem={(key, i) => (
            <List.Item
              actions={
                canManage
                  ? [
                      <Tooltip title="Move up" key="up">
                        <Button
                          type="text"
                          size="small"
                          aria-label={`Move ${nameOf(key)} up`}
                          icon={<ArrowUpOutlined />}
                          disabled={i === 0 || savingChain}
                          onClick={() => move(i, -1)}
                        />
                      </Tooltip>,
                      <Tooltip title="Move down" key="down">
                        <Button
                          type="text"
                          size="small"
                          aria-label={`Move ${nameOf(key)} down`}
                          icon={<ArrowDownOutlined />}
                          disabled={i === chain.length - 1 || savingChain}
                          onClick={() => move(i, 1)}
                        />
                      </Tooltip>,
                      <Tooltip title="Remove from chain" key="rm">
                        <Button
                          type="text"
                          size="small"
                          danger
                          aria-label={`Remove ${nameOf(key)} from chain`}
                          icon={<DeleteOutlined />}
                          disabled={savingChain}
                          onClick={() => remove(i)}
                        />
                      </Tooltip>,
                    ]
                  : []
              }
            >
              <Space>
                <Typography.Text>{`${i + 1}. ${nameOf(key)}`}</Typography.Text>
                {!configuredOf(key) && (
                  <Typography.Text type="warning" className="text-xs">
                    (not configured)
                  </Typography.Text>
                )}
              </Space>
            </List.Item>
          )}
        />
      )}

      {canManage && notInChain.length > 0 && (
        <Select
          className="mt-3"
          style={{ width: '100%' }}
          placeholder="Add a provider to the chain…"
          value={null}
          disabled={savingChain}
          onChange={(key: string) => add(key)}
          options={notInChain.map(p => ({ value: p.key, label: p.display_name }))}
        />
      )}
    </Card>
  )
}
