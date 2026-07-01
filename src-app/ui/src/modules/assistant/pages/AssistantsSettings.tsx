import { Bot, Pencil, Trash2 } from 'lucide-react'
import {
  Button,
  Card,
  Descriptions,
  Separator,
  Empty,
  Confirm,
  Tag,
  Text,
  message,
} from '@/components/ui'
import { ListPagination } from '@/components/common/ListPagination'
import { Loading } from '@/core/components/Loading'
import { useEffect } from 'react'
import { Stores } from '@/modules/assistant/stores'
import { Can, usePermission } from '@/core/permissions'
import { AddButton } from '@/modules/settings/components/AddButton'
import { Permissions, type Assistant } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { AssistantFormDrawer } from '@/modules/assistant/components/AssistantFormDrawer'

export function AssistantsSettings() {
  // Store state
  const {
    assistants,
    total: totalAssistants,
    currentPage: storePage,
    pageSize: storePageSize,
    loading,
    error,
  } = Stores.TemplateAssistants

  const canEdit = usePermission(Permissions.AssistantsTemplateEdit)
  const canDelete = usePermission(Permissions.AssistantsTemplateDelete)

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.TemplateAssistants.clearTemplateAssistantsStoreError()
    }
  }, [error, message])

  const handleDelete = async (assistant: Assistant) => {
    try {
      await Stores.TemplateAssistants.deleteTemplateAssistant(assistant.id)
      message.success('Assistant deleted successfully')
    } catch (error) {
      console.error('Failed to delete assistant:', error)
      // Error is handled by the store
    }
  }

  const handleEdit = (assistant: Assistant) => {
    Stores.AssistantDrawer.openAssistantDrawer(assistant, true)
  }

  const handleCreate = () => {
    Stores.AssistantDrawer.openAssistantDrawer(null, true)
  }

  const getAssistantActions = (assistant: Assistant) => {
    const actions: React.ReactNode[] = []

    if (canEdit) {
      actions.push(
        <Button
          key="edit"
          data-testid={`template-assistant-${assistant.id}-edit`}
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
          data-testid={`template-assistant-${assistant.id}-delete-confirm`}
          title="Delete Assistant"
          description="Are you sure you want to delete this assistant?"
          onConfirm={() => handleDelete(assistant)}
          okText="Delete"
          cancelText="Cancel"
        >
          <Button data-testid={`template-assistant-${assistant.id}-delete`} variant="ghost" icon={<Trash2 />}>
            Delete
          </Button>
        </Confirm>,
      )
    }

    return actions.filter(Boolean)
  }

  const handlePageChange = (page: number, size?: number) => {
    const newPageSize = size || storePageSize
    const newPage = size && size !== storePageSize ? 1 : page // Reset to page 1 if page size changes

    Stores.TemplateAssistants.loadTemplateAssistants(newPage, newPageSize)
  }

  return (
    <SettingsPageContainer
      title="Assistant Templates"
      subtitle="Manage template assistants. Default assistants are automatically cloned for new users."
    >
      <div>
        <Card
          data-testid="template-assistants-card"
          title="Template Assistants"
          extra={
            <Can permission={Permissions.AssistantsTemplateCreate}>
              <AddButton
                label="Create assistant"
                onClick={handleCreate}
                data-testid="template-assistants-create-btn"
              />
            </Can>
          }
        >
          {loading ? (
            <Loading />
          ) : assistants.length === 0 ? (
            <div>
              <Empty data-testid="template-assistants-empty" description="No assistants yet — use the New Assistant button above to create one." />
            </div>
          ) : (
            <div>
              {assistants.map((assistant, index) => (
                <div
                  key={assistant.id}
                  data-test-assistant-id={`template-assistant-${assistant.id}`}
                >
                  <div className="flex items-start gap-3 flex-wrap">
                    {/* Assistant Info */}
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-2 flex-wrap">
                        <div className="flex-1 min-w-48">
                          <div className="flex gap-2 items-center">
                            <Bot />
                            <Text className="font-medium">
                              {assistant.name}
                            </Text>
                            {assistant.is_default && (
                              <Tag data-testid={`template-assistant-${assistant.id}-default-tag`} tone="success">Default</Tag>
                            )}
                            {!assistant.enabled && (
                              <Tag data-testid={`template-assistant-${assistant.id}-inactive-tag`} tone="error">Inactive</Tag>
                            )}
                          </div>
                        </div>
                        <div className="flex gap-1 items-center justify-end">
                          {getAssistantActions(assistant)}
                        </div>
                      </div>

                      <Descriptions
                        data-testid={`template-assistant-${assistant.id}-desc`}
                        size="sm"
                        column={3}
                        items={[
                          { key: 'description', label: 'Description', children: assistant.description || 'No description' },
                          { key: 'createdBy', label: 'Created By', children: assistant.created_by ? 'User' : 'System' },
                          { key: 'created', label: 'Created', children: new Date(assistant.created_at).toLocaleDateString() },
                        ]}
                        className="[&_.label]:text-xs [&_.content]:text-xs"
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
            <>
              <Separator className="mb-4" />
              <ListPagination
                data-testid="template-assistants-pagination"
                current={storePage}
                total={totalAssistants}
                pageSize={storePageSize}
                onChange={(page) => handlePageChange(page, storePageSize)}
                onPageSizeChange={(size) => handlePageChange(1, size)}
                aria-label="Assistants pagination"
              />
            </>
          )}
        </Card>

        <AssistantFormDrawer />
      </div>
    </SettingsPageContainer>
  )
}
