import { useEffect, useState } from 'react'
import {
  Button,
  Card,
  Descriptions,
  Divider,
  Dropdown,
  Empty,
  Flex,
  Form,
  Input,
  InputNumber,
  Pagination,
  Popconfirm,
  Select,
  Spin,
  Tag,
  Tooltip,
  Typography,
  message,
} from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  PlusOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { UserMemory } from '@/api-client/types'

const { Text } = Typography
const { Search } = Input

const READ_PERM = Permissions.MemoryRead
const WRITE_PERM = Permissions.MemoryWrite

/**
 * Per-user memory list with CRUD + filters + export.
 *
 * Hidden if no `memory::read`. Write controls (Add, Edit, Delete,
 * Delete-all) hidden if no `memory::write`. Read-only viewers see
 * the table + filters + export but no mutation affordances.
 */
export function MyMemoriesSection() {
  const canRead = usePermission(READ_PERM)
  const canWrite = usePermission(WRITE_PERM)
  const {
    memories,
    loading,
    searchQuery,
    kindFilter,
    sourceFilter,
    total: totalMemories,
    currentPage: storePage,
    pageSize: storePageSize,
  } = Stores.Memories
  const [editing, setEditing] = useState<UserMemory | null>(null)
  const [creating, setCreating] = useState(false)

  const handlePageChange = (page: number, size?: number) => {
    const nextSize = size || storePageSize
    // Reset to page 1 when the user changes page size — matches
    // UsersSettings / UserGroupsSettings behavior.
    const nextPage = size && size !== storePageSize ? 1 : page
    Stores.Memories.load(nextPage, nextSize)
  }

  // Filtering moved to the server — `memories` already reflects
  // searchQuery / kindFilter / sourceFilter via the store setters.
  // Alias kept so the downstream render code doesn't have to change.
  const filtered = memories
  // Reference the filter vars so eslint/TS don't flag them as
  // unused — they ARE consumed (by the controlled Search/Select
  // values just below), the linter just can't tell at this scope.
  void searchQuery
  void kindFilter
  void sourceFilter

  if (!canRead) return null

  const exportMenu = {
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
  }

  return (
    <Card
      title="My memories"
      extra={
        canWrite ? (
          <Tooltip title="Add memory">
            <Button
              type="text"
              icon={<PlusOutlined />}
              onClick={() => setCreating(true)}
              aria-label="Add memory"
            />
          </Tooltip>
        ) : null
      }
    >
      {/*
        Filter + action toolbar — responsive. Search grows to fill,
        Kind/Source selects keep a sensible min-width, Export and
        Delete-all hug the right edge. `flex-wrap` lets controls
        stack on narrow widths (≤sm) instead of overflowing.
      */}
      {/* mb-3 (12px) below the toolbar. Inline style as a belt-and-
        * suspenders in case Tailwind doesn't pick the class up for
        * some reason — antd Flex doesn't reset margins, but visual
        * verification showed the class wasn't applying. */}
      <Flex
        wrap="wrap"
        gap="small"
        align="center"
        style={{ marginBottom: 12 }}
      >
        <Search
          placeholder="Search content"
          allowClear
          onChange={(e) => Stores.Memories.setSearchQuery(e.target.value)}
          style={{ minWidth: 200, flex: '1 1 240px', maxWidth: 360 }}
        />
        <Select
          placeholder="Kind"
          allowClear
          value={kindFilter ?? undefined}
          onChange={(v) => Stores.Memories.setKindFilter(v ?? null)}
          style={{ flex: '0 1 160px', minWidth: 120 }}
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
          style={{ flex: '0 1 160px', minWidth: 120 }}
          options={[
            { value: 'manual', label: 'Manual' },
            { value: 'extraction', label: 'Auto-extracted' },
            { value: 'mcp_tool', label: 'Assistant tool' },
          ]}
        />
        {/* Spacer pushes Export/Delete to the right when there's
          * room; on narrow viewports they wrap to the next line
          * naturally. */}
        <div className="flex-1" />
        <Dropdown menu={exportMenu}>
          <Button icon={<DownloadOutlined />}>Export</Button>
        </Dropdown>
        {canWrite && (
          <Popconfirm
            title="Delete all memories?"
            description="This is permanent and cannot be undone."
            okText="Delete"
            okButtonProps={{ danger: true }}
            onConfirm={async () => {
              try {
                const n = await Stores.Memories.removeAll()
                message.success(`Deleted ${n} memories`)
              } catch (error) {
                message.error(
                  error instanceof Error
                    ? error.message
                    : 'Delete-all failed.',
                )
              }
            }}
          >
            <Button danger>Delete all</Button>
          </Popconfirm>
        )}
      </Flex>

      {loading && filtered.length === 0 ? (
        <div className="flex justify-center py-6">
          <Spin />
        </div>
      ) : filtered.length === 0 ? (
        <Empty description="No memories yet" />
      ) : (
        <Flex className="flex-col gap-4">
          <div>
            {filtered.map((row, index) => (
              <div key={row.id} data-memory-id={row.id}>
                <div className="flex items-start gap-3 flex-wrap">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-2 flex-wrap-reverse">
                      <div className="flex-1 min-w-48">
                        <Text className="block">{row.content}</Text>
                      </div>
                      {canWrite && (
                        <div className="flex gap-1 items-center justify-end">
                          <Tooltip title="Edit memory">
                            <Button
                              type="text"
                              size="small"
                              icon={<EditOutlined />}
                              onClick={() => setEditing(row)}
                              aria-label="Edit memory"
                            />
                          </Tooltip>
                          <Popconfirm
                            title="Delete this memory?"
                            okText="Delete"
                            okButtonProps={{ danger: true }}
                            onConfirm={async () => {
                              try {
                                await Stores.Memories.remove(row.id)
                                message.success('Memory deleted')
                              } catch (error) {
                                message.error(
                                  error instanceof Error
                                    ? error.message
                                    : 'Delete failed.',
                                )
                              }
                            }}
                          >
                            <Tooltip title="Delete memory">
                              <Button
                                type="text"
                                size="small"
                                danger
                                icon={<DeleteOutlined />}
                                aria-label={`Delete memory ${row.id}`}
                              />
                            </Tooltip>
                          </Popconfirm>
                        </div>
                      )}
                    </div>

                    <Descriptions
                      size="small"
                      column={{ xs: 1, sm: 2, md: 4 }}
                      colon={false}
                      styles={{
                        label: { fontSize: '12px' },
                        content: { fontSize: '12px' },
                      }}
                    >
                      <Descriptions.Item label="Kind">
                        <Tag className="!m-0">{row.kind}</Tag>
                      </Descriptions.Item>
                      <Descriptions.Item label="Source">
                        <Tag
                          className="!m-0"
                          color={
                            row.source === 'manual'
                              ? 'blue'
                              : row.source === 'extraction'
                                ? 'green'
                                : 'purple'
                          }
                        >
                          {row.source === 'mcp_tool' ? 'tool' : row.source}
                        </Tag>
                      </Descriptions.Item>
                      <Descriptions.Item label="Importance">
                        {row.importance}
                      </Descriptions.Item>
                      <Descriptions.Item label="Recalls">
                        {row.recall_count}
                      </Descriptions.Item>
                      <Descriptions.Item label="Updated" span={{ xs: 1, sm: 2, md: 4 }}>
                        {new Date(row.updated_at).toLocaleString()}
                      </Descriptions.Item>
                    </Descriptions>
                  </div>
                </div>
                {index < filtered.length - 1 && (
                  <Divider className="my-4" />
                )}
              </div>
            ))}
          </div>
        </Flex>
      )}

      {totalMemories > 0 && (
        <>
          <Divider className="!my-3" />
          <Flex justify="end">
            <Pagination
              current={storePage}
              total={totalMemories}
              pageSize={storePageSize}
              showSizeChanger
              showQuickJumper
              showTotal={(total, range) =>
                `${range[0]}-${range[1]} of ${total} memories`
              }
              onChange={handlePageChange}
              onShowSizeChange={handlePageChange}
              pageSizeOptions={['5', '10', '20', '50']}
            />
          </Flex>
        </>
      )}

      {canWrite && (
        <>
          <CreateMemoryDrawer
            open={creating}
            onClose={() => setCreating(false)}
          />
          <EditMemoryDrawer row={editing} onClose={() => setEditing(null)} />
        </>
      )}
    </Card>
  )
}

function exportMemories(rows: UserMemory[], format: 'json' | 'csv') {
  const filename = `ziee-memories-${new Date().toISOString().slice(0, 10)}.${format}`
  let blob: Blob
  if (format === 'json') {
    blob = new Blob([JSON.stringify(rows, null, 2)], {
      type: 'application/json',
    })
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
      if (
        s.includes(',') ||
        s.includes('"') ||
        s.includes('\n') ||
        s.includes('\r')
      ) {
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

function CreateMemoryDrawer({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  const [form] = Form.useForm<{
    content: string
    importance: number
    kind: string
  }>()
  const { saving } = Stores.Memories

  const handleSubmit = async (values: {
    content: string
    importance: number
    kind: string
  }) => {
    try {
      await Stores.Memories.create(
        values.content,
        values.importance,
        values.kind,
      )
      form.resetFields()
      onClose()
      message.success('Memory added')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to add memory.',
      )
    }
  }

  return (
    <Drawer
      open={open}
      title="Add memory"
      onClose={onClose}
      size={600}
      extra={
        <Button type="primary" loading={saving} onClick={() => form.submit()}>
          Add
        </Button>
      }
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
          <Input.TextArea
            rows={4}
            placeholder="One sentence, third-person about you"
          />
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

function EditMemoryDrawer({
  row,
  onClose,
}: {
  row: UserMemory | null
  onClose: () => void
}) {
  const [form] = Form.useForm<{
    content: string
    importance: number
    kind: string
  }>()
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
    try {
      await Stores.Memories.update(row.id, values)
      onClose()
      message.success('Memory updated')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to update memory.',
      )
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
