import { useEffect, useMemo, useState } from 'react'
import {
  Typography,
  Table,
  Input,
  Select,
  Button,
  Tag,
  Modal,
  Form,
  InputNumber,
  Drawer,
  Space,
  Empty,
  Popconfirm,
  message,
} from 'antd'
import { DeleteOutlined, EditOutlined, PlusOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { AppLayout } from '@/modules/layouts/app-layout'
import type { UserMemoryRow } from '@/modules/memory/stores/Memories.store'

const { Title, Paragraph, Text } = Typography
const { Search } = Input

export function MemoriesPage() {
  const { memories, loading, searchQuery, kindFilter, sourceFilter } =
    Stores.Memories
  const [editing, setEditing] = useState<UserMemoryRow | null>(null)
  const [creating, setCreating] = useState(false)

  useEffect(() => {
    Stores.Memories.load()
  }, [])

  const filtered = useMemo(() => {
    return memories.filter((m) => {
      if (kindFilter && m.kind !== kindFilter) return false
      if (sourceFilter && m.source !== sourceFilter) return false
      if (searchQuery && !m.content.toLowerCase().includes(searchQuery.toLowerCase()))
        return false
      return true
    })
  }, [memories, kindFilter, sourceFilter, searchQuery])

  return (
    <AppLayout>
      <div className="max-w-5xl mx-auto p-6">
        <div className="flex items-center justify-between mb-4">
          <Title level={3} className="!mb-0">
            My Memories
          </Title>
          <Space>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={() => setCreating(true)}
            >
              Add memory
            </Button>
            <Popconfirm
              title="Delete all memories?"
              description="This is permanent and cannot be undone."
              okType="danger"
              okText="Delete all"
              onConfirm={async () => {
                const n = await Stores.Memories.removeAll()
                message.success(`Deleted ${n} memories`)
              }}
            >
              <Button danger>Delete all</Button>
            </Popconfirm>
          </Space>
        </div>

        <Paragraph type="secondary">
          The assistant uses these facts to personalize responses across
          conversations. You can add memories manually here, or let the
          assistant capture them automatically (turn that on in Memory
          settings).
        </Paragraph>

        <div className="flex gap-2 mb-4">
          <Search
            placeholder="Search content"
            allowClear
            onChange={(e) => Stores.Memories.setSearchQuery(e.target.value)}
            style={{ maxWidth: 300 }}
          />
          <Select
            placeholder="Kind"
            allowClear
            value={kindFilter ?? undefined}
            onChange={(v) => Stores.Memories.setKindFilter(v ?? null)}
            style={{ minWidth: 140 }}
            options={[
              { value: 'preference', label: 'Preference' },
              { value: 'fact', label: 'Fact' },
              { value: 'goal', label: 'Goal' },
              { value: 'relationship', label: 'Relationship' },
              { value: 'other', label: 'Other' },
            ]}
          />
          <Select
            placeholder="Source"
            allowClear
            value={sourceFilter ?? undefined}
            onChange={(v) => Stores.Memories.setSourceFilter(v ?? null)}
            style={{ minWidth: 140 }}
            options={[
              { value: 'manual', label: 'Manual' },
              { value: 'extraction', label: 'Auto-extracted' },
              { value: 'mcp_tool', label: 'Assistant tool' },
            ]}
          />
        </div>

        {filtered.length === 0 && !loading ? (
          <Empty description="No memories yet. Add one or enable auto-extraction." />
        ) : (
          <Table<UserMemoryRow>
            dataSource={filtered}
            rowKey="id"
            loading={loading}
            pagination={{ pageSize: 25 }}
            columns={[
              {
                title: 'Content',
                dataIndex: 'content',
                ellipsis: true,
              },
              {
                title: 'Kind',
                dataIndex: 'kind',
                width: 110,
                render: (v: string) => <Tag>{v}</Tag>,
              },
              {
                title: 'Source',
                dataIndex: 'source',
                width: 120,
                render: (v: string) => (
                  <Tag color={v === 'manual' ? 'blue' : v === 'extraction' ? 'green' : 'purple'}>
                    {v === 'mcp_tool' ? 'tool' : v}
                  </Tag>
                ),
              },
              {
                title: 'Importance',
                dataIndex: 'importance',
                width: 100,
              },
              {
                title: 'Recalls',
                dataIndex: 'recall_count',
                width: 90,
              },
              {
                title: 'Updated',
                dataIndex: 'updated_at',
                width: 170,
                render: (v: string) => (
                  <Text type="secondary">{new Date(v).toLocaleString()}</Text>
                ),
              },
              {
                title: '',
                key: 'actions',
                width: 100,
                render: (_, row) => (
                  <Space>
                    <Button
                      icon={<EditOutlined />}
                      size="small"
                      onClick={() => setEditing(row)}
                    />
                    <Popconfirm
                      title="Delete this memory?"
                      okType="danger"
                      onConfirm={async () => {
                        const ok = await Stores.Memories.remove(row.id)
                        if (ok) message.success('Memory deleted')
                      }}
                    >
                      <Button icon={<DeleteOutlined />} size="small" danger />
                    </Popconfirm>
                  </Space>
                ),
              },
            ]}
          />
        )}

        <CreateMemoryModal
          open={creating}
          onClose={() => setCreating(false)}
        />
        <EditMemoryDrawer
          row={editing}
          onClose={() => setEditing(null)}
        />
      </div>
    </AppLayout>
  )
}

function CreateMemoryModal({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  const [form] = Form.useForm()
  const { saving } = Stores.Memories
  return (
    <Modal
      open={open}
      title="Add memory"
      onCancel={onClose}
      onOk={async () => {
        const values = await form.validateFields()
        const row = await Stores.Memories.create(
          values.content,
          values.importance,
          values.kind,
        )
        if (row) {
          form.resetFields()
          onClose()
          message.success('Memory added')
        }
      }}
      confirmLoading={saving}
      okText="Add"
    >
      <Form form={form} layout="vertical" initialValues={{ importance: 50, kind: 'fact' }}>
        <Form.Item
          name="content"
          label="Content"
          rules={[
            { required: true, message: 'Required' },
            { max: 4000, message: 'Max 4000 chars' },
          ]}
        >
          <Input.TextArea rows={4} placeholder="One sentence, third-person about you" />
        </Form.Item>
        <Form.Item name="kind" label="Kind">
          <Select
            options={[
              { value: 'preference', label: 'Preference' },
              { value: 'fact', label: 'Fact' },
              { value: 'goal', label: 'Goal' },
              { value: 'relationship', label: 'Relationship' },
              { value: 'other', label: 'Other' },
            ]}
          />
        </Form.Item>
        <Form.Item name="importance" label="Importance (0-100)">
          <InputNumber min={0} max={100} />
        </Form.Item>
      </Form>
    </Modal>
  )
}

function EditMemoryDrawer({
  row,
  onClose,
}: {
  row: UserMemoryRow | null
  onClose: () => void
}) {
  const [form] = Form.useForm()
  const { saving } = Stores.Memories

  useEffect(() => {
    if (row) {
      form.setFieldsValue({
        content: row.content,
        importance: row.importance,
        kind: row.kind,
      })
    }
  }, [row])

  return (
    <Drawer
      open={!!row}
      title="Edit memory"
      onClose={onClose}
      width={500}
      extra={
        <Button
          type="primary"
          loading={saving}
          onClick={async () => {
            if (!row) return
            const values = await form.validateFields()
            const updated = await Stores.Memories.update(row.id, values)
            if (updated) {
              onClose()
              message.success('Memory updated')
            }
          }}
        >
          Save
        </Button>
      }
    >
      <Form form={form} layout="vertical">
        <Form.Item
          name="content"
          label="Content"
          rules={[{ required: true, max: 4000 }]}
        >
          <Input.TextArea rows={6} />
        </Form.Item>
        <Form.Item name="kind" label="Kind">
          <Select
            options={[
              { value: 'preference', label: 'Preference' },
              { value: 'fact', label: 'Fact' },
              { value: 'goal', label: 'Goal' },
              { value: 'relationship', label: 'Relationship' },
              { value: 'other', label: 'Other' },
            ]}
          />
        </Form.Item>
        <Form.Item name="importance" label="Importance (0-100)">
          <InputNumber min={0} max={100} />
        </Form.Item>
      </Form>
    </Drawer>
  )
}
