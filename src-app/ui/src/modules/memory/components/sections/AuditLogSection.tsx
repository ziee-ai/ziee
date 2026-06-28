import {
  Button,
  Card,
  Empty,
  Form,
  InputNumber,
  Spin,
  Table,
  Tag,
  Typography,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { MemoryAuditEntry } from '@/api-client/types'

const { Text, Paragraph } = Typography

const READ_PERM = Permissions.MemoryRead

interface FormValues {
  limit: number
}

/**
 * Append-only audit log of memory operations on the viewing user's
 * account. Read-only; hidden if no `memory::read`.
 */
export function AuditLogSection() {
  const canRead = usePermission(READ_PERM)
  const { entries, loading, limit } = Stores.MemoryAudit
  const [form] = Form.useForm<FormValues>()

  if (!canRead) return null

  const handleSubmit = (values: FormValues) => {
    Stores.MemoryAudit.setLimit(values.limit)
  }

  return (
    <Card title="Audit log">
      <Paragraph type="secondary" className="!mb-3 text-sm">
        Append-only record of every memory operation on your account.
        Use this to audit what the auto-extractor captured, what the
        assistant&rsquo;s tools added, and what you deleted (and when).
      </Paragraph>

      <Form
        form={form}
        layout="inline"
        initialValues={{ limit }}
        onFinish={handleSubmit}
        className="mb-4"
      >
        <Form.Item name="limit" label="Show last">
          <InputNumber min={1} max={500} />
        </Form.Item>
        <Form.Item>
          <Button type="primary" htmlType="submit">
            Apply
          </Button>
        </Form.Item>
      </Form>

      {loading ? (
        <div className="flex justify-center py-6">
          <Spin />
        </div>
      ) : entries.length === 0 ? (
        <Empty description="No audit entries yet" />
      ) : (
        <Table<MemoryAuditEntry>
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
