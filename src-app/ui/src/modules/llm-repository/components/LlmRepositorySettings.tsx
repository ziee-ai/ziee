import { CloudDownload, Pencil, Plus, Trash2 } from 'lucide-react'
import {
  Alert,
  Button,
  Card,
  Empty,
  Flex,
  Pagination,
  Switch,
  Tooltip,
} from '@/components/ui'
import {
  Text,
  message,
  Separator,
  Confirm,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions, type LlmRepository } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'

export function LlmRepositorySettings() {
  // Stores
  const {
    repositories,
    testing,
    total: totalRepositories,
    currentPage: storePage,
    pageSize: storePageSize,
  } = Stores.LlmRepository

  const canEdit = usePermission(Permissions.LlmRepositoriesEdit)
  const canDelete = usePermission(Permissions.LlmRepositoriesDelete)

  const handlePageChange = (page: number, size?: number) => {
    const nextSize = size || storePageSize
    // Reset to page 1 when the user changes page size — matches
    // UsersSettings / UserGroupsSettings behavior.
    const nextPage = size && size !== storePageSize ? 1 : page
    Stores.LlmRepository.loadLlmRepositories(nextPage, nextSize)
  }

  const testRepositoryConnection = async (repository: LlmRepository) => {
    // Check if repository has credentials configured
    if (!Stores.LlmRepository.llmRepositoryHasCredentials(repository)) {
      message.warning(
        'Please configure authentication credentials for this repository first',
      )
      return
    }

    try {
      // Use the persisted by-id endpoint instead of the stateless
      // `/test` route. The by-id path:
      //   - reads the decrypted secret from the row (no need to
      //     POST it from the client),
      //   - records the test outcome to `last_connection_*` columns,
      //   - emits `repository.updated` / `repository.auto_disabled`
      //     events that the list/store auto-reload on,
      //   - participates in the cross-surface `testing` mutex so a
      //     drawer probe + list-page probe can't race.
      // The stateless `testLlmRepositoryConnection` is reserved for
      // the Add-Repository drawer path where the row doesn't exist yet.
      const result = await Stores.LlmRepository.testLlmRepositoryById(
        repository.id,
        {},
      )

      if (result.success) {
        message.success(
          result.message || `Connection to ${repository.name} successful!`,
        )
      } else {
        message.error(
          result.message || `Connection to ${repository.name} failed`,
        )
      }
    } catch (error: any) {
      console.error('Repository connection test failed:', error)
      message.error(error?.message || `Connection to ${repository.name} failed`)
    }
  }

  // Repository management functions
  const handleAddRepository = () => {
    Stores.LlmRepositoryDrawer.openDrawer()
  }

  const handleEditRepository = (repository: LlmRepository) => {
    Stores.LlmRepositoryDrawer.openDrawer(repository)
  }

  const handleDeleteRepository = async (repositoryId: string) => {
    // Don't allow deleting built-in repositories
    const repo = repositories.find(r => r.id === repositoryId)
    if (repo?.built_in) {
      message.warning('Built-in repositories cannot be deleted')
      return
    }

    try {
      await Stores.LlmRepository.deleteLlmRepository(repositoryId)
      message.success('Repository removed successfully')
    } catch (error: any) {
      console.error('Failed to delete repository:', error)
      message.error(error?.message || 'Failed to delete repository')
    }
  }

  const handleToggleRepository = async (
    repositoryId: string,
    enabled: boolean,
  ) => {
    try {
      await Stores.LlmRepository.updateLlmRepository(repositoryId, { enabled })
      message.success(
        `Repository ${enabled ? 'enabled' : 'disabled'} successfully`,
      )
    } catch (error: any) {
      // The backend's `enforce_on_update_transition` returns 400 with
      // a `LLM_REPOSITORY_ENABLE_FAILED_HEALTH_CHECK` code + readable
      // reason when an enable-transition probe fails. The store
      // re-fetches the row in this scenario (auto_disabled event), so
      // the Switch will snap back + the Alert will render. We just
      // surface the reason in a longer-lived toast.
      console.error('Failed to toggle repository:', error)
      const reason = error?.message || 'Failed to toggle repository'
      message.error(reason)
    }
  }

  const getRepositoryActions = (repository: LlmRepository) => {
    const actions: React.ReactNode[] = []

    // Enable/disable switch — requires edit
    if (canEdit) {
      actions.push(
        <Switch
          key="enable"
          data-testid={`llmrepo-toggle-${repository.id}`}
          className="!mr-2"
          checked={repository.enabled}
          onChange={checked => handleToggleRepository(repository.id, checked)}
          aria-label={`Toggle ${repository.name} repository`}
        />,
      )
    }

    // Test connection — read is enough to view, but testing hits
    // outbound auth so gate behind edit to stay consistent with the
    // backend's update endpoint.
    if (canEdit) {
      actions.push(
        <Button
          key="test"
          data-testid={`llmrepo-test-btn-${repository.id}`}
          variant="outline"
          icon={<CloudDownload />}
          loading={testing}
          onClick={() => testRepositoryConnection(repository)}
        >
          Test
        </Button>,
      )
    }

    if (canEdit) {
      actions.push(
        <Button
          key="edit"
          data-testid={`llmrepo-edit-btn-${repository.id}`}
          variant="outline"
          icon={<Pencil />}
          onClick={() => handleEditRepository(repository)}
        >
          Edit
        </Button>,
      )
    }

    if (canDelete && !repository.built_in) {
      actions.push(
        <Confirm
          key="delete"
          data-testid={`llmrepo-delete-confirm-${repository.id}`}
          title="Are you sure?"
          onConfirm={() => handleDeleteRepository(repository.id)}
          okText="Delete"
          cancelText="Cancel"
          okButtonProps={{ danger: true }}
        >
          <Button data-testid={`llmrepo-delete-btn-${repository.id}`} variant="destructive" icon={<Trash2 />}>
            Delete
          </Button>
        </Confirm>,
      )
    }

    return actions.filter(Boolean)
  }

  return (
    <SettingsPageContainer
      title="LLM Repositories"
      subtitle="Manage your LLM model repositories and their authentication settings"
    >
      {/* Model Repositories */}
      <Card
        title="Model Repositories"
        data-testid="llmrepo-card"
        extra={
          <Can permission={Permissions.LlmRepositoriesCreate}>
            <Tooltip title="Add repository">
              <Button
                data-testid="llmrepo-add-btn"
                variant="outline"
                size="icon"
                icon={<Plus />}
                onClick={handleAddRepository}
                aria-label="Add repository"
                tooltip="Add repository"
              />
            </Tooltip>
          </Can>
        }
      >
        <Flex className="flex-col gap-4">
          <div>
            {repositories.length === 0 ? (
              <Empty
                data-testid="llmrepo-empty"
                description="No repositories yet"
              >
                <Text type="secondary">Add a repository to get started</Text>
              </Empty>
            ) : (
              <div>
                {repositories.map((repository, index) => (
                  <div key={repository.id}>
                    <div className="flex items-start gap-3 flex-wrap">
                      {/* Repository Info */}
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-2 flex-wrap-reverse">
                          <div className="flex-1 min-w-48">
                            <Flex align="center" gap="small">
                              <Text className="font-medium">
                                {repository.name}
                              </Text>
                              {repository.built_in && (
                                <Text type="secondary" className="text-xs">
                                  (Built-in)
                                </Text>
                              )}
                              {!repository.enabled && (
                                <Text type="secondary" className="text-xs">
                                  (Disabled)
                                </Text>
                              )}
                            </Flex>
                          </div>
                          <div className="flex gap-1 items-center justify-end">
                            {getRepositoryActions(repository)}
                          </div>
                        </div>

                        <div className="space-y-1">
                          <Text type="secondary" className="block">
                            {repository.url}
                          </Text>
                          <Text type="secondary" className="text-xs block">
                            Authentication:{' '}
                            {repository.auth_type === 'none'
                              ? 'None'
                              : repository.auth_type === 'api_key'
                                ? 'API Key'
                                : repository.auth_type === 'basic_auth'
                                  ? 'Basic Auth'
                                  : repository.auth_type === 'bearer_token'
                                    ? 'Bearer Token'
                                    : repository.auth_type}
                          </Text>
                        </div>

                        {/* Surface the last probe's failure reason
                            inline as an Alert so the operator
                            immediately sees what went wrong without
                            hovering for a tooltip. Mirrors the
                            McpServerCard pattern at lines 323-337 of
                            McpServerCard.tsx — only renders for the
                            unhealthy case; healthy / untested rows
                            don't need an attention-grabbing block. */}
                        {repository.last_health_check_status ===
                          'unhealthy' && (
                          <Alert
                            data-testid={`llmrepo-health-alert-${repository.id}`}
                            tone="error"
                            className="!mt-2"
                            closeLabel="Close"
                            onClose={() => {}}
                            title={
                              repository.last_health_check_at
                                ? `Connection test failed at ${new Date(
                                    repository.last_health_check_at,
                                  ).toLocaleString()}`
                                : 'Connection test failed'
                            }
                            description={
                              repository.last_health_check_reason ??
                              'No reason recorded.'
                            }
                          />
                        )}
                      </div>
                    </div>
                    {index < repositories.length - 1 && (
                      <Separator className="my-4" />
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          {totalRepositories > 0 && (
            <>
              <Separator className="!my-3" />
              <Flex justify="end">
                <Pagination
              data-testid="llmrepo-pagination"
              previousLabel="Previous page" nextLabel="Next page" pageLabel={(p) => `Page ${p}`} aria-label="Pagination"
                  current={storePage}
                  total={totalRepositories}
                  pageSize={storePageSize}
                  showSizeChanger
              pageSizeLabel="Page size"
              onPageSizeChange={(size: number) => handlePageChange(1, size)}
                  showQuickJumper
              jumpLabel="Go to page"
                  showTotal={(total: number, range: [number, number]) =>
                    `${range[0]}-${range[1]} of ${total} repositories`
                  }
                  onChange={handlePageChange}
                  pageSizeOptions={[5, 10, 20, 50]}
                />
              </Flex>
            </>
          )}
        </Flex>
      </Card>
    </SettingsPageContainer>
  )
}
