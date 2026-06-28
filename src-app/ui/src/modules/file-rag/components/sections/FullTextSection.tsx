import { useEffect } from 'react'
import { Alert, Button, Card, Divider, Flex, Form, InputNumber, Spin, Switch, message } from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

interface FormValues {
  fts_enabled: boolean
  fts_rrf_k: number
  fts_candidate_multiplier: number
  fts_min_rank: number
}

/**
 * Full-text (lexical) arm tuning. Works with no embedding model — this is the
 * day-one search experience. When semantic search is also on, the two arms are
 * fused with Reciprocal Rank Fusion (`fts_rrf_k`, `fts_candidate_multiplier`).
 */
export function FullTextSection() {
  const canReadPerm = usePermission(READ_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const canRead = canReadPerm || canManage
  const { settings, saving, error } = Stores.FileRagAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    // Don't clobber the admin's unsaved edits on a mid-edit refetch.
    if (settings && !form.isFieldsTouched()) {
      form.setFieldsValue({
        fts_enabled: settings.fts_enabled,
        fts_rrf_k: settings.fts_rrf_k,
        fts_candidate_multiplier: settings.fts_candidate_multiplier,
        fts_min_rank: settings.fts_min_rank,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Full-text search">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Full-text search"
        error={error}
        onRetry={() => Stores.FileRagAdmin.load()}
      />
    )

  const handleSubmit = async (values: FormValues) => {
    try {
      await Stores.FileRagAdmin.update({
        fts_enabled: values.fts_enabled,
        fts_rrf_k: values.fts_rrf_k,
        fts_candidate_multiplier: values.fts_candidate_multiplier,
        fts_min_rank: values.fts_min_rank,
      })
      message.success('Full-text settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save settings.',
      )
    }
  }

  return (
    <Card title="Full-text search">
      {error && (
        <Alert
          type="error"
          showIcon
          closable={{ closeIcon: true }}
          className="!mb-4"
          message={error}
        />
      )}
      <Form
        name="file-rag-admin-fts-form"
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
          name="fts_enabled"
          label="Enable full-text search"
          extra="The lexical arm. When off (and no embedder is set), semantic_search returns nothing."
          valuePropName="checked"
        >
          <Switch aria-label="Enable full-text search" />
        </Form.Item>

        <Form.Item
          name="fts_rrf_k"
          label="RRF k"
          extra="Reciprocal Rank Fusion constant for blending the vector + full-text arms. Higher = more egalitarian. Default 60 (the RRF paper)."
        >
          <InputNumber min={1} max={1000} style={{ width: 160 }} />
        </Form.Item>

        <Form.Item
          name="fts_candidate_multiplier"
          label="Candidate multiplier"
          extra="Hybrid pulls top-K × this many candidates from each arm before fusion. Higher = more recall, more DB load."
        >
          <InputNumber min={1} max={20} style={{ width: 160 }} />
        </Form.Item>

        <Form.Item
          name="fts_min_rank"
          label="Minimum rank"
          extra="ts_rank_cd cutoff. 0.0 = no filter (default). Raise to drop weak lexical matches."
        >
          <InputNumber min={0} max={1} step={0.05} style={{ width: 160 }} />
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
