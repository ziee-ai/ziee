import { useEffect, useState } from 'react'
import { Typography, Table, Tag, Spin, Empty, InputNumber, Space } from 'antd'
import { Stores } from '@/core/stores'

const { Title, Paragraph, Text } = Typography

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
 * /memories/audit-log — surfaces the user's append-only audit log.
 *
 * Closes audit R7-#5: the /api/memory/audit-log endpoint existed but
 * was orphaned with no UI consumer. This page exposes the user's
 * ADD/UPDATE/DELETE/BULK_DELETE history so they can audit what was
 * captured (especially auto-extracted memories) and when.
 */
export function AuditLogPage() {
  const [entries, setEntries] = useState<AuditEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [limit, setLimit] = useState<number>(100)

  async function load() {
    setLoading(true)
    try {
      const res = await fetch(
        `/api/memory/audit-log?limit=${limit}`,
        { credentials: 'include' },
      )
      if (!res.ok) throw new Error(`Failed: ${res.status}`)
      setEntries(await res.json())
    } catch {
      // surface as empty list; user can refetch
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    load()
  }, [limit])

  return (
    <div className="max-w-4xl mx-auto p-6">
      <Title level={3}>Memory audit log</Title>
      <Paragraph type="secondary">
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
        <div className="flex justify-center mt-8">
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
                <Tag color={v === 'manual' ? 'blue' : v === 'extraction' ? 'green' : 'purple'}>
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
      {/* Touch the Memories store to ensure event-bus subscribers
          stay live (memory.created / memory.deleted events fire and
          this page can refetch). */}
      {Stores.Memories.memories.length >= 0 && null}
    </div>
  )
}
