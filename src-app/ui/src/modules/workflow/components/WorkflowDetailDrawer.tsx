import {
  CalculatorOutlined,
  DeleteOutlined,
  ExperimentOutlined,
  PlayCircleOutlined,
} from '@ant-design/icons'
import {
  App,
  Button,
  Descriptions,
  Drawer,
  Empty,
  Popconfirm,
  Space,
  Steps,
  Tag,
  Typography,
} from 'antd'
import { useEffect, useMemo, useState } from 'react'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { DryRunPreviewDialog } from './DryRunPreviewDialog'
import { WorkflowRunDialog } from './WorkflowRunDialog'
import { WorkflowRunProgressView } from './WorkflowRunProgressView'
import { WorkflowScopeBadge } from './WorkflowScopeBadge'
import { WorkflowTestsPanel } from './WorkflowTestsPanel'
import { parseWorkflowIr } from './workflowIr'

const { Text, Title } = Typography

/**
 * Workflow detail: read-only step list (from the compiled IR when
 * present, else metadata) + Run / Dry-run / Test actions. Once a run is
 * kicked off, the live progress view renders inline.
 */
export function WorkflowDetailDrawer() {
  const { message } = App.useApp()
  const { isOpen, workflow } = Stores.WorkflowDrawer
  const canExecute = usePermission(Permissions.WorkflowsExecute)
  const canManage = usePermission(Permissions.WorkflowsInstall)
  const canManageSystem = usePermission(Permissions.WorkflowsManageSystem)

  const [runDialogOpen, setRunDialogOpen] = useState(false)
  const [dryRunOpen, setDryRunOpen] = useState(false)
  const [testsOpen, setTestsOpen] = useState(false)
  const [activeRunId, setActiveRunId] = useState<string | null>(null)

  // FE LOW-1: the drawer is a singleton bound to Stores.WorkflowDrawer; when
  // the user opens a different workflow's card while the drawer is open the
  // store swaps `workflow` without closing it. Reset the active run so a prior
  // workflow's progress view doesn't render under the new workflow's header.
  useEffect(() => {
    setActiveRunId(null)
  }, [workflow?.id])

  const { steps } = useMemo(
    () => (workflow ? parseWorkflowIr(workflow) : { inputs: [], steps: [] }),
    [workflow],
  )

  if (!workflow) {
    return (
      <Drawer
        open={isOpen}
        onClose={() => Stores.WorkflowDrawer.close()}
        closable={{ closeIcon: true }}
        size="large"
      />
    )
  }

  const editable = workflow.scope === 'system' ? canManageSystem : canManage

  const handleDelete = async () => {
    try {
      if (workflow.scope === 'system') {
        await Stores.SystemWorkflow.deleteSystemWorkflow(workflow.id)
      } else {
        await Stores.Workflow.deleteWorkflow(workflow.id)
      }
      message.success('Workflow deleted')
      Stores.WorkflowDrawer.close()
    } catch {
      message.error('Failed to delete workflow')
    }
  }

  return (
    <Drawer
      open={isOpen}
      onClose={() => {
        setActiveRunId(null)
        Stores.WorkflowDrawer.close()
      }}
      closable={{ closeIcon: true }}
      size="large"
      title={
        <Space>
          <Title level={5} className="!m-0">
            {workflow.display_name || workflow.name}
          </Title>
          <WorkflowScopeBadge scope={workflow.scope} isDev={workflow.is_dev} />
        </Space>
      }
      extra={
        editable ? (
          <Popconfirm
            title="Delete this workflow?"
            description="This removes the workflow and its extracted files."
            onConfirm={handleDelete}
            okText="Delete"
            okButtonProps={{ danger: true }}
          >
            <Button danger size="small" icon={<DeleteOutlined />}>
              Delete
            </Button>
          </Popconfirm>
        ) : null
      }
    >
      <div className="flex flex-col gap-4">
        {workflow.description && <Text>{workflow.description}</Text>}

        <Descriptions size="small" column={1} bordered>
          <Descriptions.Item label="Name">{workflow.name}</Descriptions.Item>
          {workflow.version && (
            <Descriptions.Item label="Version">
              {workflow.version}
            </Descriptions.Item>
          )}
          <Descriptions.Item label="Files">
            {workflow.file_count}
          </Descriptions.Item>
        </Descriptions>

        <Space wrap>
          {canExecute && (
            <Button
              type="primary"
              icon={<PlayCircleOutlined />}
              onClick={() => setRunDialogOpen(true)}
            >
              Run
            </Button>
          )}
          <Button
            icon={<CalculatorOutlined />}
            onClick={() => setDryRunOpen(true)}
          >
            Dry-run preview
          </Button>
          {workflow.is_dev && (
            <Button
              icon={<ExperimentOutlined />}
              onClick={() => setTestsOpen(true)}
            >
              Run tests
            </Button>
          )}
        </Space>

        {activeRunId && (
          <div className="border-t pt-3">
            <Text strong className="block mb-2">
              Run progress
            </Text>
            <WorkflowRunProgressView runId={activeRunId} />
          </div>
        )}

        <div>
          <Text strong className="block mb-2">
            Steps
          </Text>
          {steps.length > 0 ? (
            <Steps
              orientation="vertical"
              size="small"
              items={steps.map(s => ({
                status: 'wait',
                title: (
                  <Space size={8}>
                    <Text>{s.message || s.id}</Text>
                    {s.kind && <Tag className="text-xs !m-0">{s.kind}</Tag>}
                  </Space>
                ),
                description:
                  s.dependsOn && s.dependsOn.length > 0 ? (
                    <Text type="secondary" className="text-xs">
                      depends on: {s.dependsOn.join(', ')}
                    </Text>
                  ) : undefined,
              }))}
            />
          ) : (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description="Step details available after running"
            />
          )}
        </div>
      </div>

      <WorkflowRunDialog
        workflow={workflow}
        open={runDialogOpen}
        onClose={() => setRunDialogOpen(false)}
        onStarted={runId => setActiveRunId(runId)}
      />
      <DryRunPreviewDialog
        workflow={workflow}
        open={dryRunOpen}
        onClose={() => setDryRunOpen(false)}
      />
      <WorkflowTestsPanel
        workflow={workflow}
        open={testsOpen}
        onClose={() => setTestsOpen(false)}
      />
    </Drawer>
  )
}
