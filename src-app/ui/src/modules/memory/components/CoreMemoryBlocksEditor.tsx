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
} from 'antd'
import { DeleteOutlined, PlusOutlined, EditOutlined } from '@ant-design/icons'

const { Title, Paragraph, Text } = Typography

interface CoreMemoryBlock {
  id: string
  assistant_id: string
  user_id: string
  block_label: string
  content: string
  char_limit: number
  created_at: string
  updated_at: string
}

/**
 * Letta-style core-memory block editor for an assistant. Renders below
 * the assistant's regular `instructions` field on the assistant edit
 * drawer. Backed by `/api/assistants/{id}/core-memory` (GET / PUT / DELETE).
 *
 * Each user gets their own set of blocks per assistant — the UI just
 * lists what's there and lets the user add / edit / delete. Plan §9
 * Phase 6: "Assistant designer UI to set/edit blocks".
 */
export function CoreMemoryBlocksEditor({ assistantId }: { assistantId: string }) {
  const [blocks, setBlocks] = useState<CoreMemoryBlock[]>([])
  const [loading, setLoading] = useState(false)
  const [editing, setEditing] = useState<CoreMemoryBlock | null>(null)
  const [creating, setCreating] = useState(false)

  async function load() {
    setLoading(true)
    try {
      const res = await fetch(`/api/assistants/${assistantId}/core-memory`, {
        credentials: 'include',
      })
      if (!res.ok) throw new Error(`Load failed: ${res.status}`)
      setBlocks(await res.json())
    } catch (e: any) {
      message.error(e?.message || 'Failed to load core memory blocks')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    if (assistantId) load()
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

      {blocks.length === 0 && !loading ? (
        <Empty
          description="No blocks yet"
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      ) : (
        <div className="space-y-2">
          {blocks.map((b) => (
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
                      const res = await fetch(
                        `/api/assistants/${assistantId}/core-memory/${b.block_label}`,
                        { method: 'DELETE', credentials: 'include' },
                      )
                      if (res.ok || res.status === 204) {
                        message.success('Block deleted')
                        await load()
                      } else {
                        message.error('Delete failed')
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
        onSaved={load}
      />
      <BlockFormModal
        open={!!editing}
        assistantId={assistantId}
        existing={editing ?? undefined}
        onClose={() => setEditing(null)}
        onSaved={load}
      />
    </Card>
  )
}

function BlockFormModal({
  open,
  assistantId,
  existing,
  onClose,
  onSaved,
}: {
  open: boolean
  assistantId: string
  existing?: CoreMemoryBlock
  onClose: () => void
  onSaved: () => Promise<void>
}) {
  const [form] = Form.useForm()
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (open) {
      form.setFieldsValue(
        existing ?? { block_label: '', content: '', char_limit: 2000 },
      )
    }
  }, [open, existing])

  return (
    <Modal
      open={open}
      title={existing ? `Edit "${existing.block_label}"` : 'Add core memory block'}
      onCancel={onClose}
      confirmLoading={saving}
      onOk={async () => {
        const values = await form.validateFields()
        setSaving(true)
        try {
          const res = await fetch('/api/assistants/core-memory', {
            method: 'PUT',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              assistant_id: assistantId,
              block_label: values.block_label,
              content: values.content,
              char_limit: values.char_limit,
            }),
          })
          if (!res.ok) {
            const text = await res.text()
            throw new Error(text || `Save failed: ${res.status}`)
          }
          message.success(existing ? 'Block updated' : 'Block added')
          onClose()
          await onSaved()
        } catch (e: any) {
          message.error(e?.message || 'Save failed')
        } finally {
          setSaving(false)
        }
      }}
      okText={existing ? 'Save' : 'Add'}
    >
      <Form form={form} layout="vertical">
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
