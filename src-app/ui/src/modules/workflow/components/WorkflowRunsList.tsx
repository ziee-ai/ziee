import { DeleteOutlined } from '@ant-design/icons'
import { Button, Empty, List, Confirm, Space, Tag, Text, Link } from '@/components/ui'
import { useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

const STATUS_COLOR: Record<string, string> = {
  completed: 'green',
  failed: 'red',
  cancelled: 'default',
  running: 'blue',
  pending: 'gold',
}

// Trigger-source label. Covers the values migration 106's CHECK admits
// (manual | conversation | agent | mcp_tool); falls back to "Workflow page".
const INVOCATION_SOURCE_LABEL: Record<string, string> = {
  manual: 'Workflow page',
  conversation: 'Conversation',
  agent: 'Agent',
  mcp_tool: 'MCP tool',
}

/**
 * Per-workflow run history (A4). Lists the caller's own runs of a workflow with
 * a status + trigger badge; clicking a run opens the live progress view, and
 * (with execute permission) a per-row delete cascades artifacts for runs not
 * tied to a conversation. Cross-device updates arrive via the store's
 * `sync:workflow_run` subscription.
 */
export function WorkflowRunsList({
  workflowId,
  onSelectRun,
}: {
  workflowId: string
  onSelectRun: (runId: string) => void
}) {
  const navigate = useNavigate()
  const canExecute = usePermission(Permissions.WorkflowsExecute)
  const { runs, loading, deleting } = Stores.WorkflowRuns

  // Parameterized load: refetch whenever the open workflow changes. Live
  // updates ride the store's sync:workflow_run subscription.
  useEffect(() => {
    void Stores.WorkflowRuns.loadRuns(workflowId)
  }, [workflowId])

  const items = runs[workflowId] || []

  const handleDelete = async (runId: string) => {
    try {
      await Stores.WorkflowRuns.deleteRun(runId, workflowId)
    } catch (e) {
    }
  }

  if (!loading[workflowId] && items.length === 0) {
    return (
      <Empty description="No runs yet" />
    )
  }

  return (
    <List
      size="sm"
      loading={!!loading[workflowId] && items.length === 0}
      dataSource={items}
      renderItem={(run) => (
        <div
          key={run.id}
          className="cursor-pointer"
          onClick={() => onSelectRun(run.id)}
        >
          <div className="flex justify-between items-center">
            <Space size={8} wrap>
              <Tag tone={STATUS_COLOR[run.status] === 'green' ? 'success' : STATUS_COLOR[run.status] === 'red' ? 'error' : STATUS_COLOR[run.status] === 'blue' ? 'info' : STATUS_COLOR[run.status] === 'gold' ? 'warning' : undefined} className="!m-0">
                {run.status}
              </Tag>
              {run.invocation_source === 'conversation' && run.conversation_id ? (
                // Conversation-launched run: a Typography.Link (accessible
                // <a role="link">, the codebase's inline-nav idiom — cf.
                // DownloadItem) inside the badge opens the originating
                // conversation. stopPropagation so the click navigates instead of
                // firing the div's open-progress onClick.
                <Tag className="!m-0 text-xs">
                  <Link
                    href="#"
                    className="text-xs"
                    onClick={(e: React.MouseEvent) => {
                      e.stopPropagation()
                      e.preventDefault()
                      navigate(`/chat/${run.conversation_id}`)
                    }}
                  >
                    {INVOCATION_SOURCE_LABEL.conversation}
                  </Link>
                </Tag>
              ) : (
                <Tag className="!m-0 text-xs">
                  {INVOCATION_SOURCE_LABEL[run.invocation_source] ?? 'Workflow page'}
                </Tag>
              )}
              <Text type="secondary" className="text-xs">
                {new Date(run.created_at).toLocaleString()}
              </Text>
              {run.total_tokens > 0 && (
                <Text type="secondary" className="text-xs">
                  {run.total_tokens} tok
                </Text>
              )}
            </Space>
            {canExecute && (
              <Confirm
                key="del"
                title="Delete this run?"
                description="Artifacts are removed too unless the run is tied to a conversation."
                onConfirm={() => {
                  void handleDelete(run.id)
                }}
                onCancel={() => {}}
                okText="Delete"
                cancelText="Cancel"
                okButtonProps={{ danger: true }}
              >
                <Button
                  variant="destructive"
                  size="sm"
                  icon={<DeleteOutlined />}
                  loading={!!deleting[run.id]}
                  onClick={(e: React.MouseEvent) => e.stopPropagation()}
                />
              </Confirm>
            )}
          </div>
        </div>
      )}
    />
  )
}
