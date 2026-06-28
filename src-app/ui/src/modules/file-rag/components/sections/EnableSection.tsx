import { useEffect } from 'react'
import { Alert, Button, Card, Divider, Flex, Form, InputNumber, Spin, Switch, message } from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

interface FormValues {
  enabled: boolean
  default_top_k: number
}

/**
 * Master Document-RAG card: deployment-wide enable + the shared `default_top_k`
 * retrieval cap. Default is ON (full-text from day one). Per-arm toggles live
 * in their own cards below.
 */
export function EnableSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving, error } = Stores.FileRagAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    // Don't clobber the admin's unsaved edits when a refetch (e.g. a sync
    // reconnect) reloads settings mid-edit.
    if (settings && !form.isFieldsTouched()) {
      form.setFieldsValue({
        enabled: settings.enabled,
        default_top_k: settings.default_top_k,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Document search">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings) {
    return (
      <Card title="Document search">
        {error ? (
          <Alert
            type="error"
            showIcon
            title="Failed to load Document RAG admin settings"
            description={error}
          />
        ) : (
          <div className="flex justify-center py-16">
            <Spin />
          </div>
        )}
      </Card>
    )
  }

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.FileRagAdmin.update({
        enabled: values.enabled,
        default_top_k: values.default_top_k,
      })
      message.success('Document search settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save settings.',
      )
    }
  }

  return (
    <Card title="Document search">
      <Form
        name="file-rag-admin-master-form"
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
          label="Enable Document RAG deployment-wide"
          extra="On by default. When off, files are not indexed and the semantic_search tool returns a disabled note. Full-text search works immediately; semantic search additionally needs an embedding model (below)."
          valuePropName="checked"
        >
          <Switch aria-label="Enable Document RAG deployment-wide" />
        </Form.Item>

        <Form.Item
          name="default_top_k"
          label="Default top-K"
          extra="How many passages semantic_search returns when the caller doesn't specify. The model can request fewer per call; a single call returns at most 50."
        >
          <InputNumber min={1} max={50} style={{ width: 160 }} />
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
