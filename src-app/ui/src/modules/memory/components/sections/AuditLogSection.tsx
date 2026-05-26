import { useEffect, useState } from 'react'
import {
  Card,
  Empty,
  InputNumber,
  Space,
  Spin,
  Table,
  Tag,
  Typography,
} from 'antd'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const { Text, Paragraph } = Typography

const READ_PERM = Permissions.MemoryRead

interface AuditEntry {
  id: number
  user_id: string
  memory_id: string | null
  op: 'ADD' | 'UPDATE' | 'DELETE' | 'BULK_DELETE'
  source: 'extraction' | 'mcp_tool' | 'manual' | 'admin'
  content_snapshot: string | null
  actor_kind: 'user' | 'assistant' | 'admin' | 'system'
  metadata: unknown
  created_at: string
}

/**
 * Append-only audit log of memory operations on the viewing user's
 * account. Read-only; hidden if no `memory::read`.
 */
export function AuditLogSection() {
  const canRead = usePermission(READ_PERM)
  const [entries, setEntries] = useState<AuditEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [limit, setLimit] = useState<number>(100)

  useEffect(() => {
    if (!canRead) return
    let cancelled = false
    ;(async () => {
      setLoading(true)
      try {
        const res = await fetch(`/api/memory/audit-log?limit=${limit}`, {
          credentials: 'include',
        })
        if (!res.ok) throw new Error(`Failed: ${res.status}`)
        const rows: AuditEntry[] = await res.json()
        if (!cancelled) setEntries(rows)
      } catch {
        if (!cancelled) setEntries([])
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [canRead, limit])

  if (!canRead) return null

  return (
    <Card title="Audit log">
      <Paragraph type="secondary" className="!mb-3 text-sm">
        Append-only record of every memory operation on your account.
        Use this to audit what the auto-extractor captured, what the
        assistant&rsquo;s tools added, and what you deleted (and when).
      </Paragraph>

      <Space className="mb-4" align="center">
        <Text>Show last</Text>
        <InputNumber
          min={1}
          max={500}
          value={limit}
          onChange={(v) => setLimit(typeof v === 'number' ? v : 100)}
        />
        <Text>entries</Text>
      </Space>

      {loading ? (
        <div className="flex justify-center py-6">
          <Spin />
        </div>
      ) : entries.length === 0 ? (
        <Empty description="No audit entries yet" />
      ) : (
        <Table<AuditEntry>
          dataSource={entries}
          rowKey="id"
          size="middle"
          pagination={{ pageSize: 25 }}
          columns={[
            {
              title: 'When',
              dataIndex: 'created_at',
              width: 180,
              render: (v: string) => (
                <Text type="secondary">{new Date(v).toLocaleString()}</Text>
              ),
            },
            {
              title: 'Op',
              dataIndex: 'op',
              width: 130,
              render: (v: string) => {
                const color =
                  v === 'ADD'
                    ? 'green'
                    : v === 'UPDATE'
                      ? 'blue'
                      : v === 'DELETE'
                        ? 'red'
                        : 'volcano'
                return <Tag color={color}>{v}</Tag>
              },
            },
            {
              title: 'Source',
              dataIndex: 'source',
              width: 120,
              render: (v: string) => (
                <Tag
                  color={
                    v === 'manual'
                      ? 'blue'
                      : v === 'extraction'
                        ? 'green'
                        : 'purple'
                  }
                >
                  {v === 'mcp_tool' ? 'tool' : v}
                </Tag>
              ),
            },
            {
              title: 'Actor',
              dataIndex: 'actor_kind',
              width: 100,
              render: (v: string) => <Tag>{v}</Tag>,
            },
            {
              title: 'Snapshot',
              dataIndex: 'content_snapshot',
              ellipsis: true,
              render: (v: string | null) =>
                v ? <Text>{v}</Text> : <Text type="secondary">—</Text>,
            },
          ]}
        />
      )}
    </Card>
  )
}
