import { ImportOutlined } from '@ant-design/icons'
import { Button, Empty, Flex, Space, Text } from '@/components/ui'
import { useState } from 'react'
import { Permissions } from '@/api-client/types'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ImportWorkflowDialog } from '@/modules/workflow/components/ImportWorkflowDialog'
import { WorkflowDetailDrawer } from '@/modules/workflow/components/WorkflowDetailDrawer'
import { WorkflowScopeBadge } from '@/modules/workflow/components/WorkflowScopeBadge'
import { AdminWorkflowGroupAssignment } from './AdminWorkflowGroupAssignment'

/**
 * `/settings/workflows-admin` — lists system-scope workflows. Admins
 * install system workflows from the Hub (scope dropdown) or via local
 * import.
 */
export function AdminWorkflowsPage() {
  const { systemWorkflows, loading } = Stores.SystemWorkflow
  const { multiUserMode } = Stores.AppMode
  const [importOpen, setImportOpen] = useState(false)

  return (
    <SettingsPageContainer
      title="System Workflows"
      subtitle="Workflows installed for the whole deployment"
    >
      <div className="flex flex-col gap-3 h-full">
        <Flex justify="end">
          <Can permission={Permissions.WorkflowsManageSystem}>
            <Button
              icon={<ImportOutlined />}
              onClick={() => setImportOpen(true)}
            >
              Import
            </Button>
          </Can>
        </Flex>

        {loading && <Text type="secondary">Loading system workflows...</Text>}

        <div className="flex flex-col gap-3">
          {systemWorkflows.map(workflow => (
            <div
              key={workflow.id}
              className="relative overflow-hidden border rounded-lg"
              data-workflow-id={workflow.id}
            >
              <div className="overflow-hidden">
                <div
                  className="p-3 cursor-pointer"
                  onClick={() => Stores.WorkflowDrawer.open(workflow)}
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

        {!loading && systemWorkflows.length === 0 && (
          <Empty
            description="No system workflows installed"
            className="!mt-12"
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
