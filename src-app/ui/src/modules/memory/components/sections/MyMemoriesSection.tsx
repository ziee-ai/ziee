import { useEffect, useState } from 'react'
import {
  Button,
  Card,
  Confirm,
  Descriptions,
  Dropdown,
  Empty,
  Flex,
  Form,
  FormField,
  Input,
  InputNumber,
  Pagination,
  Select,
  Separator,
  Spin,
  Tag,
  Text,
  Textarea,
  Tooltip,
  message,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'

// Shared validation for the add/edit memory drawers (was antd Form.Item `rules`).
const memoryFormSchema = z.object({
  content: z.string().min(1, 'Required').max(4000, 'Max 4000 chars'),
  importance: z.number(),
  kind: z.string(),
})
import {
  Trash2,
  Download,
  Pencil,
} from 'lucide-react'
import { Stores } from '@/core/stores'
import { AddButton } from '@/modules/settings/components/AddButton'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { UserMemory } from '@/api-client/types'

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
    // Default the store-derived list: on a route where the Memories
    // store hasn't initialized yet, the Stores proxy yields `undefined`
    // for its fields, and an unguarded `memories.length`/`.map` below
    // would throw during render and white-screen the whole page (it took
    // down the desktop combined Memory page). Default to an empty list.
    memories = [],
    loading,
    searchQuery,
    kindFilter,
    sourceFilter,
    total: totalMemories = 0,
    currentPage: storePage,
    pageSize: storePageSize,
  } = Stores.Memories
  const [editing, setEditing] = useState<UserMemory | null>(null)
  const [creating, setCreating] = useState(false)

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
      data-testid="memory-my-card"
      extra={
        canWrite ? (
          <AddButton
            label="Add memory"
            onClick={() => setCreating(true)}
            data-testid="memory-add-btn"
          />
        ) : null
      }
      footer={
        <Flex justify="end" gap="small" className="w-full">
          <Dropdown items={exportMenu.items} data-testid="memory-export-dropdown">
            <Button icon={<Download />} data-testid="memory-export-btn">Export</Button>
          </Dropdown>
          {canWrite && (
            <Confirm
              title="Delete all memories?"
              data-testid="memory-delete-all-confirm"
              description="This is permanent and cannot be undone."
              okText="Delete"
              cancelText="Cancel"
              okButtonProps={{ danger: true }}
              onConfirm={async () => {
                try {
                  const n = await Stores.Memories.removeAll()
                  message.success(`Deleted ${n} memories`)
                } catch (error) {
                  message.error(
                    error instanceof Error ? error.message : 'Delete-all failed.',
                  )
                }
              }}
            >
              <Button variant="destructive" icon={<Trash2 />} data-testid="memory-delete-all-btn">Delete all</Button>
            </Confirm>
          )}
        </Flex>
      }
    >
      {/* Filter toolbar — search grows to fill; kind/source keep a min-width. */}
      <Flex
        wrap
        gap="small"
        align="center"
        className="mb-3"
      >
        <Input
          placeholder="Search content"
          allowClear
          onChange={(e) => Stores.Memories.setSearchQuery(e.target.value)}
          className="min-w-[200px] flex-1"
          data-testid="memory-search-input"
        />
        <Select
          placeholder="Kind"
          value={kindFilter ?? undefined}
          onChange={(v) => Stores.Memories.setKindFilter(v ?? null)}
          className="flex-[0_1_160px] min-w-[120px]"
          data-testid="memory-kind-filter"
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
          value={sourceFilter ?? undefined}
          onChange={(v) => Stores.Memories.setSourceFilter(v ?? null)}
          allowClear
          clearLabel="Clear source filter"
          className="flex-[0_1_160px] min-w-[120px]"
          data-testid="memory-source-filter"
          options={[
            { value: 'manual', label: 'Manual' },
            { value: 'extraction', label: 'Auto-extracted' },
            { value: 'mcp_tool', label: 'Assistant tool' },
          ]}
        />
      </Flex>

      {loading && filtered.length === 0 ? (
        <div className="flex justify-center py-6">
          <Spin label="Loading" />
        </div>
      ) : filtered.length === 0 ? (
        <Empty description="No memories yet" data-testid="memory-empty" />
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
                              variant="ghost"
                              size="default"
                              icon={<Pencil />}
                              onClick={() => setEditing(row)}
                              aria-label="Edit memory"
                              data-testid={`memory-row-edit-btn-${row.id}`}
                            />
                          </Tooltip>
                          <Confirm
                            title="Delete this memory?"
                            data-testid={`memory-row-delete-confirm-${row.id}`}
                            okText="Delete"
                            cancelText="Cancel"
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
                                variant="ghost"
                                size="default"
                                icon={<Trash2 />}
                                aria-label={`Delete memory ${row.id}`}
                                data-testid={`memory-row-delete-btn-${row.id}`}
                              />
                            </Tooltip>
                          </Confirm>
                        </div>
                      )}
                    </div>

                    <Descriptions
                      size="sm"
                      column={4}
                      data-testid={`memory-row-descriptions-${row.id}`}
                      items={[
                        {
                          key: 'kind',
                          label: 'Kind',
                          children: <Tag className="!m-0" data-testid={`memory-row-kind-tag-${row.id}`}>{row.kind}</Tag>,
                        },
                        {
                          key: 'source',
                          label: 'Source',
                          children: (
                            <Tag
                              className="!m-0"
                              data-testid={`memory-row-source-tag-${row.id}`}
                              tone={
                                row.source === 'manual'
                                  ? 'info'
                                  : row.source === 'extraction'
                                    ? 'success'
                                    : 'info'
                              }
                            >
                              {row.source === 'mcp_tool' ? 'tool' : row.source}
                            </Tag>
                          ),
                        },
                        {
                          key: 'importance',
                          label: 'Importance',
                          children: row.importance,
                        },
                        {
                          key: 'recalls',
                          label: 'Recalls',
                          children: row.recall_count,
                        },
                        {
                          key: 'updated',
                          label: 'Updated',
                          children: new Date(row.updated_at).toLocaleString(),
                          span: 4,
                        },
                      ]}
                    />
                  </div>
                </div>
                {index < filtered.length - 1 && (
                  <Separator className="my-4" />
                )}
              </div>
            ))}
          </div>
        </Flex>
      )}

      {totalMemories > 0 && (
        <>
          <Separator className="!my-3" />
          <Flex justify="end">
            <Pagination
              data-testid="memory-pagination"
              current={storePage}
              total={totalMemories}
              pageSize={storePageSize}
              showSizeChanger
              pageSizeLabel="Memories per page"
              pageSizeOptions={[5, 10, 20, 50]}
              onPageSizeChange={(size) => Stores.Memories.load(1, size)}
              showQuickJumper
              jumpLabel="Jump to page"
              showTotal={(total, range) =>
                `${range[0]}-${range[1]} of ${total} memories`
              }
              onChange={(page) => Stores.Memories.load(page, storePageSize)}
              aria-label="Memory pagination"
              previousLabel="Previous page"
              nextLabel="Next page"
              pageLabel={(p) => `Page ${p}`}
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
  const form = useForm<{
    content: string
    importance: number
    kind: string
  }>({
    resolver: zodResolver(memoryFormSchema),
    defaultValues: { importance: 50, kind: 'fact' },
  })
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
      form.reset()
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
        <Button loading={saving} onClick={() => void form.handleSubmit(handleSubmit)()} data-testid="memory-create-submit-btn">
          Add
        </Button>
      }
    >
      <Form
        form={form}
        layout="vertical"
        onSubmit={handleSubmit}
        data-testid="memory-create-form"
      >
        <FormField
          name="content"
          label="Content"
        >
          <Textarea
            rows={4}
            placeholder="One sentence, third-person about you"
            data-testid="memory-create-content-input"
          />
        </FormField>
        <FormField name="kind" label="Kind">
          <Select
            data-testid="memory-create-kind-select"
            options={[
              { value: 'preference', label: 'Preference' },
              { value: 'fact', label: 'Fact' },
              { value: 'goal', label: 'Goal' },
              { value: 'relationship', label: 'Relationship' },
              { value: 'other', label: 'Other' },
            ]}
          />
        </FormField>
        <FormField name="importance" label="Importance (0-100)">
          <InputNumber min={0} max={100} data-testid="memory-create-importance-input" />
        </FormField>
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
  const form = useForm<{
    content: string
    importance: number
    kind: string
  }>({ resolver: zodResolver(memoryFormSchema) })
  const { saving } = Stores.Memories

  useEffect(() => {
    if (row) {
      form.reset({
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
        <Button loading={saving} onClick={() => void form.handleSubmit(handleSubmit)()} data-testid="memory-edit-submit-btn">
          Save
        </Button>
      }
    >
      <Form form={form} layout="vertical" onSubmit={handleSubmit} data-testid="memory-edit-form">
        <FormField
          name="content"
          label="Content"
        >
          <Textarea rows={6} data-testid="memory-edit-content-input" />
        </FormField>
        <FormField name="kind" label="Kind">
          <Select
            data-testid="memory-edit-kind-select"
            options={[
              { value: 'preference', label: 'Preference' },
              { value: 'fact', label: 'Fact' },
              { value: 'goal', label: 'Goal' },
              { value: 'relationship', label: 'Relationship' },
              { value: 'other', label: 'Other' },
            ]}
          />
        </FormField>
        <FormField name="importance" label="Importance (0-100)">
          <InputNumber min={0} max={100} data-testid="memory-edit-importance-input" />
        </FormField>
      </Form>
    </Drawer>
  )
}
