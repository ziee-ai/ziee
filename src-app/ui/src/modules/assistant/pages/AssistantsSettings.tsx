import {
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
  RobotOutlined,
} from '@ant-design/icons'
import {
  Button,
  Card,
  Descriptions,
  Separator,
  Empty,
  Pagination,
  Confirm,
  Tag,
  Tooltip,
  Text,
  message,
} from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { useEffect } from 'react'
import { Stores } from '@/modules/assistant/stores'
import { Can, usePermission } from '@/core/permissions'
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
          variant="ghost"
          icon={<EditOutlined />}
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
          title="Delete Assistant"
          description="Are you sure you want to delete this assistant?"
          onConfirm={() => handleDelete(assistant)}
          okText="Delete"
          cancelText="Cancel"
        >
          <Button variant="destructive" icon={<DeleteOutlined />}>
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
          title="Template Assistants"
          extra={
            <Can permission={Permissions.AssistantsTemplateCreate}>
              <Tooltip content="Create assistant">
                <Button
                  variant="ghost"
                  icon={<PlusOutlined aria-hidden="true" />}
                  onClick={handleCreate}
                  aria-label="Create assistant"
                />
              </Tooltip>
            </Can>
          }
        >
          {loading ? (
            <Loading />
          ) : assistants.length === 0 ? (
            <div>
              <Empty description="No assistants yet" />
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
                            <RobotOutlined />
                            <Text className="font-medium">
                              {assistant.name}
                            </Text>
                            {assistant.is_default && (
                              <Tag tone="success">Default</Tag>
                            )}
                            {!assistant.enabled && (
                              <Tag tone="error">Inactive</Tag>
                            )}
                          </div>
                        </div>
                        <div className="flex gap-1 items-center justify-end">
                          {getAssistantActions(assistant)}
                        </div>
                      </div>

                      <Descriptions
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
              <div className="flex justify-end">
                <Pagination
              previousLabel="Previous page" nextLabel="Next page" pageLabel={(p) => `Page ${p}`}
                  current={storePage}
                  total={totalAssistants}
                  pageSize={storePageSize}
                  onChange={(page) => handlePageChange(page, storePageSize)}
                  aria-label="Assistants pagination"
                />
              </div>
            </>
          )}
        </Card>

        <AssistantFormDrawer />
      </div>
    </SettingsPageContainer>
  )
}
