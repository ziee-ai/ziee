import {
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
  RobotOutlined,
} from '@ant-design/icons'
import {
  App,
  Button,
  Card,
  Descriptions,
  Divider,
  Empty,
  Flex,
  Pagination,
  Popconfirm,
  Spin,
  Tag,
  Typography,
} from 'antd'
import { useEffect } from 'react'
import { Stores } from '@/modules/assistants/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions, type Assistant } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { AssistantFormDrawer } from '@/modules/assistants/components/AssistantFormDrawer'

const { Text } = Typography

export function AssistantsSettings() {
  const { message } = App.useApp()

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
          type="text"
          icon={<EditOutlined />}
          onClick={() => handleEdit(assistant)}
        >
          Edit
        </Button>,
      )
    }

    if (canDelete) {
      actions.push(
        <Popconfirm
          key="delete"
          title="Delete Assistant"
          description="Are you sure you want to delete this assistant?"
          onConfirm={() => handleDelete(assistant)}
          okText="Delete"
          cancelText="Cancel"
        >
          <Button type="text" danger icon={<DeleteOutlined />}>
            Delete
          </Button>
        </Popconfirm>,
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
      title="Assistants"
      subtitle="Manage template assistants. Default assistants are automatically cloned for new users."
    >
      <div>
        <Card
          title="Template Assistants"
          extra={
            <Can permission={Permissions.AssistantsTemplateCreate}>
              <Button
                type="text"
                icon={<PlusOutlined aria-hidden="true" />}
                onClick={handleCreate}
                aria-label="Create assistant"
              />
            </Can>
          }
        >
          {loading ? (
            <div className="flex justify-center py-8">
              <Spin size="large" />
            </div>
          ) : assistants.length === 0 ? (
            <div>
              <Empty description="No assistants found" />
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
                          <Flex className="gap-2 items-center">
                            <RobotOutlined />
                            <Text className="font-medium">
                              {assistant.name}
                            </Text>
                            {assistant.is_default && (
                              <Tag color="success">Default</Tag>
                            )}
                            {!assistant.enabled && (
                              <Tag color="error">Inactive</Tag>
                            )}
                          </Flex>
                        </div>
                        <div className="flex gap-1 items-center justify-end">
                          {getAssistantActions(assistant)}
                        </div>
                      </div>

                      <Descriptions
                        size="small"
                        column={{ xs: 1, sm: 2, md: 3 }}
                        colon={false}
                        styles={{
                          label: { fontSize: '12px' },
                          content: { fontSize: '12px' },
                        }}
                      >
                        <Descriptions.Item label="Description">
                          {assistant.description || 'No description'}
                        </Descriptions.Item>
                        <Descriptions.Item label="Created By">
                          {assistant.created_by ? 'User' : 'System'}
                        </Descriptions.Item>
                        <Descriptions.Item label="Created">
                          {new Date(assistant.created_at).toLocaleDateString()}
                        </Descriptions.Item>
                      </Descriptions>
                    </div>
                  </div>
                  {index < assistants.length - 1 && (
                    <Divider className="my-4" />
                  )}
                </div>
              ))}
            </div>
          )}

          {assistants.length > 0 && (
            <>
              <Divider className="mb-4" />
              <div className="flex justify-end">
                <Pagination
                  current={storePage}
                  total={totalAssistants}
                  pageSize={storePageSize}
                  showSizeChanger
                  showQuickJumper
                  showTotal={(total, range) =>
                    `${range[0]}-${range[1]} of ${total} assistants`
                  }
                  onChange={handlePageChange}
                  onShowSizeChange={handlePageChange}
                  pageSizeOptions={['5', '10', '20', '50']}
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
