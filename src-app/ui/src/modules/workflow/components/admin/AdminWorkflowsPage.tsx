import { ImportOutlined } from '@ant-design/icons'
import { Button, Card, Empty, Flex, Space, Typography } from 'antd'
import { useState } from 'react'
import { Permissions } from '@/api-client/types'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ImportWorkflowDialog } from '@/modules/workflow/components/ImportWorkflowDialog'
import { WorkflowDetailDrawer } from '@/modules/workflow/components/WorkflowDetailDrawer'
import { WorkflowScopeBadge } from '@/modules/workflow/components/WorkflowScopeBadge'
import { AdminWorkflowGroupAssignment } from './AdminWorkflowGroupAssignment'

const { Text } = Typography

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
            <Card
              key={workflow.id}
              classNames={{ body: '!p-0' }}
              className="overflow-hidden"
              data-workflow-id={workflow.id}
            >
              <div
                className="p-3 cursor-pointer"
                onClick={() => Stores.WorkflowDrawer.open(workflow)}
              >
                <Space vertical size={2}>
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
                </Space>
              </div>
              {multiUserMode && (
                <AdminWorkflowGroupAssignment workflowId={workflow.id} />
              )}
            </Card>
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
