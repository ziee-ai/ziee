import { Calculator, CirclePlay, FlaskConical, Trash2 } from 'lucide-react'
import {
  Button,
  Descriptions,
  Dialog,
  Empty,
  Space,
  Tag,
  Text,
  Title,
  message,
} from '@/components/ui'
import { useEffect, useMemo, useState } from 'react'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { DryRunPreviewDialog } from './DryRunPreviewDialog'
import { WorkflowRunDialog } from './WorkflowRunDialog'
import { WorkflowRunProgressView } from './WorkflowRunProgressView'
import { WorkflowRunsList } from './WorkflowRunsList'
import { WorkflowScopeBadge } from './WorkflowScopeBadge'
import { WorkflowTestsPanel } from './WorkflowTestsPanel'
import { parseWorkflowIr } from './workflowIr'

/**
 * Workflow detail: read-only step list (from the compiled IR when
 * present, else metadata) + Run / Dry-run / Test actions. Once a run is
 * kicked off, the live progress view renders inline.
 */
export function WorkflowDetailDrawer() {
  const { isOpen, workflow } = Stores.WorkflowDrawer
  const canExecute = usePermission(Permissions.WorkflowsExecute)
  const canManage = usePermission(Permissions.WorkflowsInstall)
  const canManageSystem = usePermission(Permissions.WorkflowsManageSystem)

  const [runDialogOpen, setRunDialogOpen] = useState(false)
  const [dryRunOpen, setDryRunOpen] = useState(false)
  const [testsOpen, setTestsOpen] = useState(false)
  // Must live above the `if (!workflow) return` early return below — a hook
  // after a conditional return changes the hook count between renders (workflow
  // null while the drawer's data loads, then non-null), tripping React #310 and
  // blanking the whole route via the error boundary.
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false)
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
      <Dialog
        data-testid="wf-detail-dialog-empty"
        open={isOpen}
        onOpenChange={(open) => { if (!open) Stores.WorkflowDrawer.close() }}
        className="!max-w-[480px]"
        title=""
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
    <Dialog
      data-testid="wf-detail-dialog"
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) {
          setActiveRunId(null)
          Stores.WorkflowDrawer.close()
        }
      }}
      className="!max-w-[480px]"
      title={
        <Space>
          <Title level={5} className="!m-0">
            {workflow.display_name || workflow.name}
          </Title>
          <WorkflowScopeBadge scope={workflow.scope} isDev={workflow.is_dev} />
        </Space>
      }
      footer={
        editable ? (
          <>
            <Button data-testid="wf-detail-delete-btn" onClick={() => setDeleteDialogOpen(true)} variant="ghost" size="default" icon={<Trash2 />}>
              Delete
            </Button>
            <Dialog
              data-testid="wf-detail-delete-dialog"
              open={deleteDialogOpen}
              onOpenChange={(open) => {
                if (!open) setDeleteDialogOpen(false)
              }}
              title="Delete this workflow?"
              description="This removes the workflow and its extracted files."
              footer={
                <div className="flex justify-end gap-2">
                  <Button data-testid="wf-detail-delete-cancel-btn" onClick={() => setDeleteDialogOpen(false)} variant="outline">
                    Cancel
                  </Button>
                  <Button data-testid="wf-detail-delete-confirm-btn" onClick={handleDelete} variant="destructive">
                    Delete
                  </Button>
                </div>
              }
            >
              <Text>This action cannot be undone.</Text>
            </Dialog>
          </>
        ) : null
      }
    >
      <div className="flex flex-col gap-4">
        {workflow.description && <Text>{workflow.description}</Text>}

        <Descriptions data-testid="wf-detail-descriptions" size="sm" column={1} bordered
          items={[
            { key: 'name', label: 'Name', children: workflow.name },
            ...(workflow.version ? [{ key: 'version', label: 'Version', children: workflow.version }] : []),
            { key: 'files', label: 'Files', children: workflow.file_count },
          ]}
        />

        <Space wrap>
          {canExecute && (
            <Button
              data-testid="wf-detail-run-btn"
              variant="default"
              icon={<CirclePlay />}
              onClick={() => setRunDialogOpen(true)}
            >
              Run
            </Button>
          )}
          <Button
            data-testid="wf-detail-dry-run-btn"
            variant="outline"
            icon={<Calculator />}
            onClick={() => setDryRunOpen(true)}
          >
            Dry-run preview
          </Button>
          {workflow.is_dev && (
            <Button
              data-testid="wf-detail-run-tests-btn"
              variant="outline"
              icon={<FlaskConical />}
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
            <div className="space-y-3">
              {steps.map((s, i) => (
                <div key={i} className="flex flex-col gap-1">
                  <Space size={8}>
                    <Text>{s.message || s.id}</Text>
                    {s.kind && <Tag variant="outline" data-testid={`wf-detail-step-kind-tag-${i}`} className="text-xs !m-0" tone="info">{s.kind}</Tag>}
                  </Space>
                  {s.dependsOn && s.dependsOn.length > 0 && (
                    <Text type="secondary" className="text-xs">
                      depends on: {s.dependsOn.join(', ')}
                    </Text>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <Empty data-testid="wf-detail-steps-empty" description="Step details available after running" />
          )}
        </div>

        <div className="border-t pt-3">
          <Text strong className="block mb-2" data-testid="wf-runs-heading">
            Runs
          </Text>
          <WorkflowRunsList
            workflowId={workflow.id}
            onSelectRun={setActiveRunId}
          />
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
    </Dialog>
  )
}
