import { useEffect } from 'react'
import { Trash2, Pencil } from 'lucide-react'
import {
  Button,
  Card,
  SectionHeader,
  Descriptions,
  Separator,
  Empty,
  ErrorState,
  Flex,
  Confirm,
  Tag,
  Text,
  message,
} from '@ziee/kit'
import { ListPagination } from '@/components/common/ListPagination'
import { Loading } from '@/core/components/Loading'
import { Stores } from '@/modules/assistant/stores'
import { Can, usePermission } from '@/core/permissions'
import { AddButton } from '@/modules/settings/components/AddButton'
import { Permissions, type Assistant } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { AssistantFormDrawer } from '@/modules/assistant/components/AssistantFormDrawer'

export function UserAssistantsSettings() {
  // Store state
  const {
    assistants,
    total: totalAssistants,
    currentPage: storePage,
    pageSize: storePageSize,
    loading,
    error,
  } = Stores.UserAssistants

  const canEdit = usePermission(Permissions.AssistantsEdit)
  const canDelete = usePermission(Permissions.AssistantsDelete)

  // A mutation failure while the list is populated → toast + clear. A cold
  // load failure (no data) persists as the in-place ErrorState below rather
  // than being cleared away into a silent empty state.
  useEffect(() => {
    if (error && assistants.length > 0) {
      message.error(error)
      Stores.UserAssistants.clearUserAssistantsStoreError()
    }
  }, [error, assistants.length])

  const handleDelete = async (assistant: Assistant) => {
    try {
      await Stores.UserAssistants.deleteUserAssistant(assistant.id)
      message.success('Assistant deleted successfully')
    } catch (error) {
      console.error('Failed to delete assistant:', error)
      // Error is surfaced via the store error effect above
    }
  }

  const handleEdit = (assistant: Assistant) => {
    Stores.AssistantDrawer.openAssistantDrawer(assistant, false)
  }

  const handleCreate = () => {
    Stores.AssistantDrawer.openAssistantDrawer(null, false)
  }

  const handlePageChange = (page: number, size?: number) => {
    const newPageSize = size || storePageSize
    const newPage = size && size !== storePageSize ? 1 : page // reset to page 1 when the page size changes

    Stores.UserAssistants.loadUserAssistants(newPage, newPageSize)
  }

  const getAssistantActions = (assistant: Assistant) => {
    const actions: React.ReactNode[] = []

    if (canEdit) {
      actions.push(
        <Button
          key="edit"
          data-testid={`user-assistant-${assistant.id}-edit`}
          variant="ghost"
          icon={<Pencil />}
          onClick={() => handleEdit(assistant)}
        >
          Edit
        </Button>,
      )
    }

    if (canDelete) {
      actions.push(
        <Confirm
          key="delete"
          data-testid={`user-assistant-${assistant.id}-delete-confirm`}
          title="Delete Assistant"
          description="Are you sure you want to delete this assistant?"
          onConfirm={() => handleDelete(assistant)}
          okText="Delete"
          cancelText="Cancel"
        >
          <Button data-testid={`user-assistant-${assistant.id}-delete`} variant="ghost" icon={<Trash2 />}>
            Delete
          </Button>
        </Confirm>,
      )
    }

    return actions.filter(Boolean)
  }

  return (
    <SettingsPageContainer
      title="Assistants"
      subtitle="Create and manage your personal assistants."
    >
      <div>
        <Card data-testid="user-assistants-card">
          {/* SectionHeader (never-wrap-with-room) instead of Card title/extra
              — fixes the mobile premature-stack of the title above the `+`
              button (taxonomy B1). */}
          <SectionHeader
            title="My Assistants"
            data-testid="user-assistants-header"
            className="mb-4"
            actions={
              <Can permission={Permissions.AssistantsCreate}>
                <AddButton
                  label="Create assistant"
                  onClick={handleCreate}
                  data-testid="user-assistants-create-btn"
                />
              </Can>
            }
          />
          {error && assistants.length === 0 ? (
            <ErrorState
              resource="assistants"
              description="Something went wrong while loading your assistants."
              details={error}
              onRetry={() =>
                Stores.UserAssistants.loadUserAssistants(storePage, storePageSize)
              }
              data-testid="user-assistants-error"
            />
          ) : loading ? (
            <Loading />
          ) : error && assistants.length === 0 ? (
            <ErrorState
              resource="assistants"
              description="Your assistants couldn't be loaded."
              details={error}
              onRetry={() => Stores.UserAssistants.loadUserAssistants(storePage, storePageSize)}
              data-testid="user-assistants-error"
            />
          ) : assistants.length === 0 ? (
            <div>
              <Empty data-testid="user-assistants-empty" description="No assistants yet — use the New Assistant button above to create one." />
            </div>
          ) : (
            <div>
              {assistants.map((assistant, index) => (
                <div
                  key={assistant.id}
                  data-test-assistant-id={`user-assistant-${assistant.id}`}
                >
                  <div className="flex items-start gap-3 flex-wrap">
                    {/* Assistant Info */}
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-2 flex-wrap">
                        <div className="flex-1 min-w-48">
                          <Flex className="gap-2 items-center">
                            <Text className="font-medium">{assistant.name}</Text>
                            {assistant.is_default && (
                              <Tag variant="outline" data-testid={`user-assistant-${assistant.id}-default-tag`} tone="success">Default</Tag>
                            )}
                            {!assistant.enabled && (
                              <Tag variant="outline" data-testid={`user-assistant-${assistant.id}-inactive-tag`} tone="error">Inactive</Tag>
                            )}
                          </Flex>
                        </div>
                        <div className="flex flex-wrap gap-1 items-center justify-end">
                          {getAssistantActions(assistant)}
                        </div>
                      </div>

                      <Descriptions
                        data-testid={`user-assistant-${assistant.id}-desc`}
                        size="sm"
                        items={[
                          {
                            key: 'description',
                            label: 'Description',
                            children: assistant.description || 'No description',
                          },
                          {
                            key: 'created',
                            label: 'Created',
                            children: new Date(assistant.created_at).toLocaleDateString(),
                          },
                        ]}
                      />
                    </div>
                  </div>
                  {index < assistants.length - 1 && (
                    <Separator className="my-4" />
                  )}
                </div>
              ))}
            </div>
          )}

          {assistants.length > 0 && (
            <ListPagination
              data-testid="user-assistants-pagination"
              current={storePage}
              total={totalAssistants}
              pageSize={storePageSize}
              onChange={(page) => handlePageChange(page, storePageSize)}
              onPageSizeChange={(size) => handlePageChange(1, size)}
              aria-label="Assistants pagination"
            />
          )}
        </Card>

        <AssistantFormDrawer />
      </div>
    </SettingsPageContainer>
  )
}
