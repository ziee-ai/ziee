import { useState } from 'react'
import { Button, Card, Empty, InputNumber, Spin } from '@/components/ui'
import { Table, Tag, Text, Paragraph } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { MemoryAuditEntry } from '@/api-client/types'

const READ_PERM = Permissions.MemoryRead

/**
 * Append-only audit log of memory operations on the viewing user's
 * account. Read-only; hidden if no `memory::read`.
 */
export function AuditLogSection() {
  const canRead = usePermission(READ_PERM)
  const { entries, loading, limit } = Stores.MemoryAudit
  const [pendingLimit, setPendingLimit] = useState<number>(limit)

  if (!canRead) return null

  return (
    <Card title="Audit log" data-testid="memory-audit-card">
      <Paragraph type="secondary" className="!mb-3 text-sm">
        Append-only record of every memory operation on your account.
        Use this to audit what the auto-extractor captured, what the
        assistant&rsquo;s tools added, and what you deleted (and when).
      </Paragraph>

      <div className="mb-4 flex flex-nowrap items-center gap-2">
        <Text className="whitespace-nowrap">Show last</Text>
        <InputNumber
          data-testid="memory-audit-limit-input"
          aria-label="Number of audit-log entries to show"
          min={1}
          max={500}
          value={pendingLimit}
          onChange={v => setPendingLimit(typeof v === 'number' ? v : 100)}
          suffix="entries"
          className="w-40"
        />
        <Button
          data-testid="memory-audit-limit-apply"
          onClick={() => Stores.MemoryAudit.setLimit(pendingLimit)}
        >
          Apply
        </Button>
      </div>

      {loading ? (
        <div className="flex justify-center py-6">
          <Spin label="Loading" />
        </div>
      ) : entries.length === 0 ? (
        <Empty description="No audit entries yet" data-testid="memory-audit-empty" />
      ) : (
        <Table<MemoryAuditEntry>
          data-testid="memory-audit-table"
          dataSource={entries}
          rowKey="id"
          columns={[
            {
              key: 'created_at',
              title: 'When',
              dataIndex: 'created_at',
              width: 180,
              render: (record: MemoryAuditEntry) => (
                <Text type="secondary">{new Date(record.created_at).toLocaleString()}</Text>
              ),
            },
            {
              key: 'op',
              title: 'Op',
              dataIndex: 'op',
              width: 130,
              render: (record: MemoryAuditEntry) => {
                const v = record.op
                const tone =
                  v === 'ADD'
                    ? 'success'
                    : v === 'UPDATE'
                      ? 'info'
                      : v === 'DELETE'
                        ? 'error'
                        : 'warning'
                return <Tag variant="outline" data-testid={`memory-audit-status-${v}`} tone={tone}>{v}</Tag>
              },
            },
            {
              key: 'source',
              title: 'Source',
              dataIndex: 'source',
              width: 120,
              render: (record: MemoryAuditEntry) => {
                const v = record.source
                const tone =
                  v === 'manual'
                    ? 'info'
                    : v === 'extraction'
                      ? 'success'
                      : 'info'
                return (
                  <Tag variant="outline" data-testid={`memory-audit-source-${v}`} tone={tone}>
                    {v === 'mcp_tool' ? 'tool' : v}
                  </Tag>
                )
              },
            },
            {
              key: 'actor_kind',
              title: 'Actor',
              dataIndex: 'actor_kind',
              width: 100,
              render: (record: MemoryAuditEntry) => {
                return <Tag variant="outline" data-testid={`memory-audit-actor-${record.actor_kind}`}>{record.actor_kind}</Tag>
              },
            },
            {
              key: 'content_snapshot',
              title: 'Snapshot',
              dataIndex: 'content_snapshot',
              width: 200,
              render: (record: MemoryAuditEntry) => {
                const v = record.content_snapshot
                return v ? <Text>{v}</Text> : <Text type="secondary">—</Text>
              },
            },
          ]}
        />
      )}
    </Card>
  )
}
