import { DeleteOutlined } from '@ant-design/icons'
import { App, Button, Empty, List, Popconfirm, Space, Tag, Typography } from 'antd'
import { useEffect } from 'react'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

const { Text } = Typography

const STATUS_COLOR: Record<string, string> = {
  completed: 'green',
  failed: 'red',
  cancelled: 'default',
  running: 'blue',
  pending: 'gold',
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
  const { message } = App.useApp()
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
      message.success('Run deleted')
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to delete run')
    }
  }

  if (!loading[workflowId] && items.length === 0) {
    return (
      <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="No runs yet" />
    )
  }

  return (
    <List
      size="small"
      loading={!!loading[workflowId] && items.length === 0}
      dataSource={items}
      renderItem={run => (
        <List.Item
          className="cursor-pointer"
          onClick={() => onSelectRun(run.id)}
          actions={
            canExecute
              ? [
                  <Popconfirm
                    key="del"
                    title="Delete this run?"
                    description="Artifacts are removed too unless the run is tied to a conversation."
                    onConfirm={e => {
                      e?.stopPropagation()
                      void handleDelete(run.id)
                    }}
                    onCancel={e => e?.stopPropagation()}
                    okText="Delete"
                    okButtonProps={{ danger: true }}
                  >
                    <Button
                      danger
                      size="small"
                      type="text"
                      icon={<DeleteOutlined />}
                      loading={!!deleting[run.id]}
                      onClick={e => e.stopPropagation()}
                    />
                  </Popconfirm>,
                ]
              : undefined
          }
        >
          <Space size={8} wrap>
            <Tag color={STATUS_COLOR[run.status] || 'default'} className="!m-0">
              {run.status}
            </Tag>
            <Tag className="!m-0 text-xs">
              {run.invocation_source === 'conversation'
                ? 'Conversation'
                : 'Workflow page'}
            </Tag>
            <Text type="secondary" className="text-xs">
              {new Date(run.created_at).toLocaleString()}
            </Text>
            {run.total_tokens > 0 && (
              <Text type="secondary" className="text-xs">
                {run.total_tokens} tok
              </Text>
            )}
          </Space>
        </List.Item>
      )}
    />
  )
}
