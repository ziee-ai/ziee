import { Import as ImportIcon, Workflow as WorkflowIcon } from 'lucide-react'
import { Button, Card, Empty, Flex, Space, Text } from '@/components/ui'
import { useState } from 'react'
import { Permissions } from '@/api-client/types'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ImportWorkflowDialog } from './ImportWorkflowDialog'
import { WorkflowDetailDrawer } from './WorkflowDetailDrawer'
import { WorkflowScopeBadge } from './WorkflowScopeBadge'

/**
 * `/workflows` page — lists the user's own + accessible system
 * workflows with a scope badge. Clicking a card opens the detail
 * drawer (steps + run / dry-run / test).
 */
export function WorkflowsList() {
  const { workflows, loading } = Stores.Workflow
  const [importOpen, setImportOpen] = useState(false)

  return (
    <SettingsPageContainer
      title="Workflows"
      subtitle="Declarative multi-step LLM chains you can run on demand"
    >
      <div className="flex flex-col gap-3 h-full">
        <Flex justify="end">
          <Can permission={Permissions.WorkflowsInstall}>
            <Button
              data-testid="wf-list-import-btn"
              icon={<ImportIcon />}
              onClick={() => setImportOpen(true)}
            >
              Import
            </Button>
          </Can>
        </Flex>

        {loading && <Text type="secondary">Loading workflows...</Text>}

        <div className="flex flex-col gap-3">
          {workflows.map(workflow => (
            <Card
              key={workflow.id}
              data-testid={`wf-list-card-${workflow.id}`}
              hoverable
              size="sm"
              onClick={() => Stores.WorkflowDrawer.open(workflow)}
              data-workflow-id={workflow.id}
              title={
                <Space size={8}>
                  <WorkflowIcon />
                  <Text strong>{workflow.display_name || workflow.name}</Text>
                  <WorkflowScopeBadge
                    scope={workflow.scope}
                    isDev={workflow.is_dev}
                  />
                </Space>
              }
            >
              {workflow.description && (
                <Text type="secondary" className="text-xs" ellipsis>
                  {workflow.description}
                </Text>
              )}
            </Card>
          ))}
        </div>

        {!loading && workflows.length === 0 && (
          <Empty
            data-testid="wf-list-empty"
            description="No workflows installed yet — browse the Hub to install one"
            className="!mt-12"
          />
        )}

        <WorkflowDetailDrawer />
        <ImportWorkflowDialog
          open={importOpen}
          onClose={() => setImportOpen(false)}
        />
      </div>
    </SettingsPageContainer>
  )
}
