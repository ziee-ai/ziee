import { useEffect } from 'react'
import { Empty, Switch, Table, Tag, Typography } from 'antd'
import type { ColumnsType } from 'antd/es/table'
import { Stores } from '@/core/stores'
import { type McpToolCall } from '@/api-client/types'

const { Text, Paragraph } = Typography

const STATUS_COLOR: Record<string, string> = {
  completed: 'green',
  failed: 'red',
  timeout: 'orange',
  cancelled: 'default',
}

const SOURCE_COLOR: Record<string, string> = {
  chat: 'blue',
  rest: 'geekblue',
  always: 'purple',
  approval: 'gold',
  sampling: 'cyan',
  workflow: 'magenta',
}

/**
 * Per-server MCP tool-call history. Rendered as a tab inside McpServerDrawer
 * (edit mode). Reads the shared McpToolCalls store, which refetches live on
 * the `sync:mcp_tool_call` event so a call appears without a manual reload.
 */
export function McpToolCallsTab({ serverId }: { serverId: string }) {
  const { calls, total, currentPage, pageSize, loading, hideBuiltIn, error } =
    Stores.McpToolCalls

  // (Re)load this server's calls on mount / when the drawer swaps servers.
  useEffect(() => {
    void Stores.McpToolCalls.loadCalls(serverId, 1)
  }, [serverId])

  const columns: ColumnsType<McpToolCall> = [
    {
      title: 'Time',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      render: (v: string) => new Date(v).toLocaleString(),
    },
    {
      title: 'Tool',
      dataIndex: 'tool_name',
      key: 'tool_name',
      render: (v: string, row: McpToolCall) => (
        <span>
          {v}
          {row.is_built_in ? (
            <Tag className="ml-1" data-testid="mcp-tool-call-builtin-tag">
              built-in
            </Tag>
          ) : null}
        </span>
      ),
    },
    {
      title: 'Status',
      dataIndex: 'status',
      key: 'status',
      width: 110,
      render: (v: string, row: McpToolCall) => (
        <Tag color={row.is_error ? 'red' : (STATUS_COLOR[v] ?? 'default')}>
          {v}
        </Tag>
      ),
    },
    {
      title: 'Source',
      dataIndex: 'source',
      key: 'source',
      width: 110,
      render: (v: string) => <Tag color={SOURCE_COLOR[v] ?? 'default'}>{v}</Tag>,
    },
    {
      title: 'Duration',
      dataIndex: 'duration_ms',
      key: 'duration_ms',
      width: 100,
      render: (v: number | null) => (v == null ? '—' : `${v} ms`),
    },
  ]

  return (
    <div className="flex flex-col gap-3" data-testid="mcp-tool-calls-tab">
      <div className="flex justify-end items-center gap-2">
        <Text type="secondary">Hide built-in</Text>
        <Switch
          size="small"
          checked={hideBuiltIn}
          onChange={v => Stores.McpToolCalls.setHideBuiltIn(v)}
          data-testid="mcp-tool-calls-hide-builtin"
        />
      </div>
      {error ? (
        <Text type="danger" data-testid="mcp-tool-calls-error">
          {error}
        </Text>
      ) : null}
      <Table<McpToolCall>
        rowKey="id"
        size="small"
        loading={loading}
        dataSource={calls}
        columns={columns}
        locale={{
          emptyText: (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description="No tool calls recorded yet"
            />
          ),
        }}
        expandable={{
          expandRowByClick: true,
          expandedRowRender: (row: McpToolCall) => (
            <div
              className="flex flex-col gap-2"
              data-testid="mcp-tool-call-detail"
            >
              <div>
                <Text strong>Arguments</Text>
                <Paragraph className="!mb-2">
                  <pre className="text-xs whitespace-pre-wrap">
                    {JSON.stringify(row.arguments_json, null, 2)}
                  </pre>
                </Paragraph>
              </div>
              <div>
                <Text strong>Result</Text>
                {row.error_message ? (
                  <Paragraph type="danger" className="!mb-0">
                    {row.error_message}
                  </Paragraph>
                ) : (
                  <Paragraph className="!mb-0">
                    <pre className="text-xs whitespace-pre-wrap">
                      {JSON.stringify(row.result_json, null, 2)}
                    </pre>
                  </Paragraph>
                )}
              </div>
            </div>
          ),
        }}
        pagination={{
          current: currentPage,
          pageSize,
          // Server-side total — the hide-built-in filter is applied server-side
          // (via the is_built_in query param), so total/pages stay consistent.
          total,
          showSizeChanger: true,
          pageSizeOptions: ['10', '20', '50'],
          onChange: (page: number, size: number) =>
            Stores.McpToolCalls.setPage(page, size),
        }}
      />
    </div>
  )
}
