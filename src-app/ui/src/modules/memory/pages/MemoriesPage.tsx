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
import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  PlusOutlined,
} from '@ant-design/icons'
import { Dropdown } from 'antd'
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
            <Dropdown
              menu={{
                items: [
                  {
                    key: 'json',
                    label: 'Export as JSON',
                    onClick: () => exportMemories(memories, 'json'),
                  },
                  {
                    key: 'csv',
                    label: 'Export as CSV',
                    onClick: () => exportMemories(memories, 'csv'),
                  },
                ],
              }}
            >
              <Button icon={<DownloadOutlined />}>Export</Button>
            </Dropdown>
            <Popconfirm
              title="Delete all memories?"
              description="This is permanent and cannot be undone."
              okText="Delete"
              okButtonProps={{ danger: true }}
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
          <Empty description="No memories yet" />
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
                      okText="Delete"
                      okButtonProps={{ danger: true }}
                      onConfirm={async () => {
                        const ok = await Stores.Memories.remove(row.id)
                        if (ok) message.success('Memory deleted')
                      }}
                    >
                      <Button
                        icon={<DeleteOutlined />}
                        size="small"
                        danger
                        aria-label={`Delete memory ${row.id}`}
                      />
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

function exportMemories(rows: UserMemoryRow[], format: 'json' | 'csv') {
  const filename = `ziee-memories-${new Date().toISOString().slice(0, 10)}.${format}`
  let blob: Blob
  if (format === 'json') {
    blob = new Blob([JSON.stringify(rows, null, 2)], { type: 'application/json' })
  } else {
    const header = [
      'id',
      'content',
      'kind',
      'source',
      'importance',
      'confidence',
      'recall_count',
      'created_at',
      'updated_at',
    ].join(',')
    const escape = (v: unknown): string => {
      const s = String(v ?? '')
      // RFC-4180: quote if cell contains comma, quote, or any line
      // ending (LF, CR, or CRLF). Audit R7-#1.
      if (s.includes(',') || s.includes('"') || s.includes('\n') || s.includes('\r')) {
        return `"${s.replace(/"/g, '""')}"`
      }
      return s
    }
    const lines = rows.map((r) =>
      [
        r.id,
        r.content,
        r.kind,
        r.source,
        r.importance,
        r.confidence,
        r.recall_count,
        r.created_at,
        r.updated_at,
      ]
        .map(escape)
        .join(','),
    )
    blob = new Blob([[header, ...lines].join('\n')], { type: 'text/csv' })
  }
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

function CreateMemoryModal({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  const [form] = Form.useForm<{ content: string; importance: number; kind: string }>()
  const { saving } = Stores.Memories

  const handleSubmit = async (values: {
    content: string
    importance: number
    kind: string
  }) => {
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
  }

  return (
    <Modal
      open={open}
      title="Add memory"
      onCancel={onClose}
      onOk={() => form.submit()}
      confirmLoading={saving}
      okText="Add"
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{ importance: 50, kind: 'fact' }}
        onFinish={handleSubmit}
      >
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
  const [form] = Form.useForm<{ content: string; importance: number; kind: string }>()
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

  const handleSubmit = async (values: {
    content: string
    importance: number
    kind: string
  }) => {
    if (!row) return
    const updated = await Stores.Memories.update(row.id, values)
    if (updated) {
      onClose()
      message.success('Memory updated')
    }
  }

  return (
    <Drawer
      open={!!row}
      title="Edit memory"
      onClose={onClose}
      size={600}
      extra={
        <Button type="primary" loading={saving} onClick={() => form.submit()}>
          Save
        </Button>
      }
    >
      <Form form={form} layout="vertical" onFinish={handleSubmit}>
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
