import { useEffect, useState } from 'react'
import {
  Typography,
  Card,
  Input,
  Button,
  Modal,
  Form,
  InputNumber,
  Popconfirm,
  message,
  Space,
  Empty,
  Spin,
} from 'antd'
import { DeleteOutlined, PlusOutlined, EditOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { CoreMemoryBlock } from '@/api-client/types'

const { Title, Paragraph, Text } = Typography

/**
 * Letta-style core-memory block editor for an assistant. Renders below
 * the assistant's regular `instructions` field on the assistant edit
 * drawer. Backed by `/api/assistants/{id}/core-memory` (GET / PUT / DELETE).
 *
 * Each user gets their own set of blocks per assistant — the UI just
 * lists what's there and lets the user add / edit / delete. Plan §9
 * Phase 6: "Assistant designer UI to set/edit blocks".
 */
export function CoreMemoryBlocksEditor({
  assistantId,
}: {
  assistantId: string
}) {
  const { blocksByAssistant, loadingByAssistant } = Stores.CoreMemoryBlocks
  const blocks = blocksByAssistant[assistantId] ?? []
  const loading = loadingByAssistant[assistantId] ?? false
  const [editing, setEditing] = useState<CoreMemoryBlock | null>(null)
  const [creating, setCreating] = useState(false)

  useEffect(() => {
    if (assistantId) Stores.CoreMemoryBlocks.load(assistantId)
  }, [assistantId])

  return (
    <Card
      title={
        <Space>
          <Title level={5} className="!mb-0">
            Core memory blocks
          </Title>
        </Space>
      }
      extra={
        <Button
          type="primary"
          size="small"
          icon={<PlusOutlined />}
          onClick={() => setCreating(true)}
        >
          Add block
        </Button>
      }
    >
      <Paragraph type="secondary" className="!mb-3 text-xs">
        Core memory blocks (Letta-style) are prepended to the assistant&rsquo;s
        system prompt on every turn. Use them for persona / human
        context / standing instructions you want always in view —
        unlike auto-extracted memories, these are explicit and never
        get retrieved by similarity.
      </Paragraph>

      {blocks.length === 0 && loading ? (
        <div className="flex justify-center py-6">
          <Spin />
        </div>
      ) : blocks.length === 0 && !loading ? (
        <Empty
          description="No blocks yet"
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      ) : (
        <div className="space-y-2">
          {blocks.map(b => (
            <Card key={b.id} size="small">
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <Text strong>{b.block_label}</Text>
                  <Paragraph
                    type="secondary"
                    className="!mt-1 !mb-0"
                    ellipsis={{ rows: 2 }}
                  >
                    {b.content}
                  </Paragraph>
                </div>
                <Space>
                  <Button
                    icon={<EditOutlined />}
                    size="small"
                    onClick={() => setEditing(b)}
                  />
                  <Popconfirm
                    title="Delete this block?"
                    description={`The "${b.block_label}" block will be removed from this assistant's core memory.`}
                    okText="Delete"
                    okButtonProps={{ danger: true }}
                    onConfirm={async () => {
                      try {
                        await Stores.CoreMemoryBlocks.remove(
                          assistantId,
                          b.block_label,
                        )
                        message.success('Block deleted')
                      } catch (error) {
                        message.error(
                          error instanceof Error
                            ? error.message
                            : 'Delete failed',
                        )
                      }
                    }}
                  >
                    <Button
                      icon={<DeleteOutlined />}
                      size="small"
                      danger
                      aria-label={`Delete block ${b.block_label}`}
                    />
                  </Popconfirm>
                </Space>
              </div>
            </Card>
          ))}
        </div>
      )}

      <BlockFormModal
        open={creating}
        assistantId={assistantId}
        onClose={() => setCreating(false)}
      />
      <BlockFormModal
        open={!!editing}
        assistantId={assistantId}
        existing={editing ?? undefined}
        onClose={() => setEditing(null)}
      />
    </Card>
  )
}

function BlockFormModal({
  open,
  assistantId,
  existing,
  onClose,
}: {
  open: boolean
  assistantId: string
  existing?: CoreMemoryBlock
  onClose: () => void
}) {
  const { loadingByAssistant } = Stores.CoreMemoryBlocks
  const saving = loadingByAssistant[assistantId] ?? false
  const [form] = Form.useForm<{
    block_label: string
    content: string
    char_limit: number
  }>()

  useEffect(() => {
    if (open) {
      form.setFieldsValue(
        existing ?? { block_label: '', content: '', char_limit: 2000 },
      )
    }
  }, [open, existing])

  const handleSubmit = async (values: {
    block_label: string
    content: string
    char_limit: number
  }) => {
    try {
      await Stores.CoreMemoryBlocks.upsert({
        assistant_id: assistantId,
        block_label: values.block_label,
        content: values.content,
        char_limit: values.char_limit,
      })
      message.success(existing ? 'Block updated' : 'Block added')
      onClose()
    } catch (error) {
      message.error(error instanceof Error ? error.message : 'Save failed')
    }
  }

  return (
    <Modal
      open={open}
      title={
        existing ? `Edit "${existing.block_label}"` : 'Add core memory block'
      }
      onCancel={onClose}
      confirmLoading={saving}
      onOk={() => form.submit()}
      okText={existing ? 'Save' : 'Add'}
    >
      <Form form={form} layout="vertical" onFinish={handleSubmit}>
        <Form.Item
          name="block_label"
          label="Label"
          rules={[
            { required: true, message: 'Required' },
            {
              pattern: /^[a-z0-9_-]{1,64}$/,
              message: '1-64 chars, lowercase letters, digits, _ or -',
            },
          ]}
        >
          <Input
            disabled={!!existing /* label is the natural key; can't rename */}
            placeholder="persona"
          />
        </Form.Item>
        <Form.Item
          name="content"
          label="Content"
          rules={[{ required: true, max: 50_000 }]}
        >
          <Input.TextArea
            rows={6}
            placeholder="Always-in-context content. The assistant will see this prepended to every system prompt."
          />
        </Form.Item>
        <Form.Item
          name="char_limit"
          label="Soft char limit"
          extra="Advisory; the LLM may exceed when writing back. Used as a hint in the system prompt."
        >
          <InputNumber min={1} max={50_000} />
        </Form.Item>
      </Form>
    </Modal>
  )
}
