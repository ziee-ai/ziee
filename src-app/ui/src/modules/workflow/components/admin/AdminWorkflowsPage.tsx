import { Import as ImportIcon } from 'lucide-react'
import { Button, Empty, ErrorState, Flex, Space, Text } from '@ziee/kit'
import { ListPagination } from '@/components/common/ListPagination'
import { useEffect, useState } from 'react'
import { Permissions } from '@/api-client/permissions'
import { Can } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ImportWorkflowDialog } from '@/modules/workflow/components/ImportWorkflowDialog'
import { WorkflowDetailDrawer } from '@/modules/workflow/components/WorkflowDetailDrawer'
import { WorkflowScopeBadge } from '@/modules/workflow/components/WorkflowScopeBadge'
import { AdminWorkflowGroupAssignment } from './AdminWorkflowGroupAssignment'
import { SystemWorkflow } from '@/modules/workflow/stores/systemWorkflow'
import { WorkflowDrawer } from '@/modules/workflow/stores/workflowDrawer'

/**
 * `/settings/workflows-admin` — lists system-scope workflows. Admins
 * install system workflows from the Hub (scope dropdown) or via local
 * import.
 */
export function AdminWorkflowsPage() {
  const { systemWorkflows, loading, error } = SystemWorkflow
  const { multiUserMode } = Stores.AppMode
  const [importOpen, setImportOpen] = useState(false)

  // Client-side pagination (the store loads the full list via listSystem()).
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const total = systemWorkflows.length
  // Snap back to a valid page if the list shrinks (e.g. after a delete).
  useEffect(() => {
    const maxPage = Math.max(1, Math.ceil(total / pageSize))
    if (page > maxPage) setPage(maxPage)
  }, [total, pageSize, page])
  const pagedWorkflows = systemWorkflows.slice((page - 1) * pageSize, page * pageSize)

  return (
    <SettingsPageContainer
      data-testid="wf-admin-page-title"
      title="System Workflows"
      subtitle="Workflows installed for the whole deployment"
    >
      <div className="flex flex-col gap-3">
        <Flex justify="end">
          <Can permission={Permissions.WorkflowsManageSystem}>
            <Button
              data-testid="wf-admin-import-btn"
              icon={<ImportIcon />}
              onClick={() => setImportOpen(true)}
            >
              Import
            </Button>
          </Can>
        </Flex>

        {loading && !error && (
          <Text type="secondary">Loading system workflows...</Text>
        )}

        <div className="flex flex-col gap-3">
          {pagedWorkflows.map(workflow => (
            <div
              key={workflow.id}
              className="relative overflow-hidden border rounded-lg"
              data-workflow-id={workflow.id}
            >
              <div className="overflow-hidden">
                <div
                  className="p-3 cursor-pointer"
                  onClick={() => WorkflowDrawer.open(workflow)}
                >
                  <div className="flex flex-col gap-2">
                    <Space size={8}>
                      <Text strong>{workflow.display_name || workflow.name}</Text>
                      <WorkflowScopeBadge
                        scope={workflow.scope}
                        isDev={workflow.is_dev}
                      />
                    </Space>
                    {workflow.description && (
                      <Text type="secondary" className="text-xs">
                        {workflow.description}
                      </Text>
                    )}
                  </div>
                </div>
                {multiUserMode && (
                  <AdminWorkflowGroupAssignment workflowId={workflow.id} />
                )}
              </div>
            </div>
          ))}
        </div>

        {error && systemWorkflows.length === 0 ? (
          <ErrorState
            resource="system workflows"
            description="Something went wrong while loading system workflows."
            details={error}
            onRetry={() => SystemWorkflow.loadSystemWorkflows()}
            data-testid="wf-admin-error"
          />
        ) : (
          !loading &&
          systemWorkflows.length === 0 && (
            <Empty
              data-testid="wf-admin-empty"
              description="No system workflows installed"
              className="!mt-12"
            />
          )
        )}

        {total > 0 && (
          <ListPagination
            data-testid="wf-admin-pagination"
            current={page}
            total={total}
            pageSize={pageSize}
            onChange={(p: number) => setPage(p)}
            onPageSizeChange={(size: number) => { setPageSize(size); setPage(1) }}
            itemNoun="workflows"
            aria-label="System workflows pagination"
          />
        )}

        <WorkflowDetailDrawer />
        <ImportWorkflowDialog
          open={importOpen}
          onClose={() => setImportOpen(false)}
          system
        />
      </div>
    </SettingsPageContainer>
  )
}
