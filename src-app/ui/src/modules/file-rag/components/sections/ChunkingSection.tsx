import { useEffect } from 'react'
import { Alert, Button, Card, Divider, Flex, Form, InputNumber, Typography, message } from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

interface FormValues {
  chunk_chars: number
  chunk_overlap_chars: number
  max_chunks_per_file: number
}

/**
 * Chunking parameters. Changes apply to files indexed AFTER saving. Existing
 * files keep their current chunking until they're re-uploaded or edited (a new
 * version re-indexes them) — backfill does NOT re-split files that already have
 * chunks.
 */
export function ChunkingSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving } = Stores.FileRagAdmin
  const [form] = Form.useForm<FormValues>()

  useEffect(() => {
    // Don't clobber the admin's unsaved edits on a mid-edit refetch.
    if (settings && !form.isFieldsTouched()) {
      form.setFieldsValue({
        chunk_chars: settings.chunk_chars,
        chunk_overlap_chars: settings.chunk_overlap_chars,
        max_chunks_per_file: settings.max_chunks_per_file,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Chunking">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings) return null

  const handleSubmit = async (values: FormValues) => {
    if (values.chunk_overlap_chars >= values.chunk_chars) {
      message.error('Overlap must be smaller than the chunk size.')
      form.setFields([
        {
          name: 'chunk_chars',
          errors: ['Must be larger than the overlap'],
        },
        {
          name: 'chunk_overlap_chars',
          errors: ['Must be smaller than the chunk size'],
        },
      ])
      return
    }
    try {
      await Stores.FileRagAdmin.update({
        chunk_chars: values.chunk_chars,
        chunk_overlap_chars: values.chunk_overlap_chars,
        max_chunks_per_file: values.max_chunks_per_file,
      })
      message.success('Chunking settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save settings.',
      )
    }
  }

  return (
    <Card title="Chunking">
      <Typography.Paragraph type="secondary" className="!mb-3 text-sm">
        Applies to files indexed after saving; existing files keep their current
        chunking until re-uploaded or edited.
      </Typography.Paragraph>
      <Form
        name="file-rag-admin-chunking-form"
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
          name="chunk_chars"
          label="Chunk size (characters)"
          extra="Target window size per chunk. ~1200 chars ≈ 300 tokens — small enough for precise citations, large enough for coherent passages."
        >
          <InputNumber min={200} max={8000} step={100} style={{ width: 160 }} />
        </Form.Item>

        <Form.Item
          name="chunk_overlap_chars"
          label="Chunk overlap (characters)"
          extra="How much consecutive chunks overlap, so a passage split across a boundary is still retrievable. Must be smaller than the chunk size."
        >
          <InputNumber min={0} max={4000} step={50} style={{ width: 160 }} />
        </Form.Item>

        <Form.Item
          name="max_chunks_per_file"
          label="Max chunks per file"
          extra="Safety cap; a file producing more chunks than this is truncated (with a server log) to bound storage and embedding cost."
        >
          <InputNumber min={1} max={100000} step={100} style={{ width: 160 }} />
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
