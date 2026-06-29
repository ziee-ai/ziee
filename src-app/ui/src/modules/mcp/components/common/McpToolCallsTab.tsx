import { useEffect, useState } from 'react'
import {
  Empty,
  Switch,
  Table,
  Tag,
  Text,
  Paragraph,
  Pagination,
  type TableColumn,
  type TagTone,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { type McpToolCall } from '@/api-client/types'

const STATUS_TONE: Record<string, TagTone> = {
  completed: 'success',
  failed: 'error',
  timeout: 'warning',
  cancelled: 'default',
}

const SOURCE_TONE: Record<string, TagTone> = {
  chat: 'info',
  rest: 'info',
  always: 'info',
  approval: 'warning',
  sampling: 'info',
  workflow: 'error',
}

/**
 * Per-server MCP tool-call history. Rendered as a tab inside McpServerDrawer
 * (edit mode). Reads the shared McpToolCalls store, which refetches live on
 * the `sync:mcp_tool_call` event so a call appears without a manual reload.
 */
export function McpToolCallsTab({ serverId }: { serverId: string }) {
  const { calls, total, currentPage, pageSize, loading, hideBuiltIn, error } =
    Stores.McpToolCalls
  const [expandedId, setExpandedId] = useState<string | null>(null)

  // (Re)load this server's calls on mount / when the drawer swaps servers.
  useEffect(() => {
    void Stores.McpToolCalls.loadCalls(serverId, 1)
  }, [serverId])

  const columns: TableColumn<McpToolCall>[] = [
    {
      title: 'Time',
      key: 'created_at',
      width: 180,
      render: row => new Date(row.created_at).toLocaleString(),
    },
    {
      title: 'Tool',
      key: 'tool_name',
      render: row => (
        <span>
          {row.tool_name}
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
      key: 'status',
      width: 110,
      render: row => (
        <Tag tone={row.is_error ? 'error' : (STATUS_TONE[row.status] ?? 'default')} data-testid={`mcp-tool-call-status-${row.id}`}>
          {row.status}
        </Tag>
      ),
    },
    {
      title: 'Source',
      key: 'source',
      width: 110,
      render: row => (
        <Tag tone={SOURCE_TONE[row.source] ?? 'default'} data-testid={`mcp-tool-call-source-${row.id}`}>{row.source}</Tag>
      ),
    },
    {
      title: 'Duration',
      key: 'duration_ms',
      width: 100,
      render: row => (row.duration_ms == null ? '—' : `${row.duration_ms} ms`),
    },
  ]

  const expandedCall = expandedId
    ? calls.find(c => c.id === expandedId)
    : undefined

  return (
    <div className="flex flex-col gap-3" data-testid="mcp-tool-calls-tab">
      <div className="flex justify-end items-center gap-2">
        <Text type="secondary">Hide built-in</Text>
        <Switch
          size="sm"
          checked={hideBuiltIn}
          onChange={v => Stores.McpToolCalls.setHideBuiltIn(v)}
          aria-label="Hide built-in"
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
        data-testid="mcp-tool-calls-table"
        loading={loading}
        dataSource={calls}
        columns={columns}
        empty={<Empty description="No tool calls recorded yet" data-testid="mcp-tool-calls-empty" />}
        onRowClick={row =>
          setExpandedId(id => (id === row.id ? null : row.id))
        }
      />
      {expandedCall ? (
        <div
          className="flex flex-col gap-2 rounded-md border p-3"
          data-testid="mcp-tool-call-detail"
        >
          <div>
            <Text strong>Arguments</Text>
            <Paragraph className="!mb-2">
              <pre className="text-xs whitespace-pre-wrap">
                {JSON.stringify(expandedCall.arguments_json, null, 2)}
              </pre>
            </Paragraph>
          </div>
          <div>
            <Text strong>Result</Text>
            {expandedCall.error_message ? (
              <Paragraph type="danger" className="!mb-0">
                {expandedCall.error_message}
              </Paragraph>
            ) : (
              <Paragraph className="!mb-0">
                <pre className="text-xs whitespace-pre-wrap">
                  {JSON.stringify(expandedCall.result_json, null, 2)}
                </pre>
              </Paragraph>
            )}
          </div>
        </div>
      ) : null}
      <Pagination
        current={currentPage}
        pageSize={pageSize}
        total={total}
        data-testid="mcp-tool-calls-pagination"
        aria-label="Tool-call pages"
        previousLabel="Previous page"
        nextLabel="Next page"
        pageLabel={p => `Page ${p}`}
        showSizeChanger
        pageSizeOptions={[10, 20, 50]}
        pageSizeLabel="Page size"
        onPageSizeChange={size => Stores.McpToolCalls.setPage(1, size)}
        onChange={page => Stores.McpToolCalls.setPage(page, pageSize)}
      />
    </div>
  )
}
