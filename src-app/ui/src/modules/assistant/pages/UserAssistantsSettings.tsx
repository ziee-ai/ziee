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
  Popconfirm,
  Tag,
  Tooltip,
  Typography,
} from 'antd'
import { Loading } from '@/core/components/Loading'
import { useEffect, useMemo } from 'react'
import { Stores } from '@/modules/assistant/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions, type Assistant } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { AssistantFormDrawer } from '@/modules/assistant/components/AssistantFormDrawer'

const { Text } = Typography

export function UserAssistantsSettings() {
  const { message } = App.useApp()

  // Store state
  const { assistants: assistantsMap, loading, error } = Stores.UserAssistants

  const assistants = useMemo(
    () => Array.from(assistantsMap.values()),
    [assistantsMap],
  )

  const canEdit = usePermission(Permissions.AssistantsEdit)
  const canDelete = usePermission(Permissions.AssistantsDelete)

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.UserAssistants.clearUserAssistantsStoreError()
    }
  }, [error, message])

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

  return (
    <SettingsPageContainer
      title="Assistants"
      subtitle="Create and manage your personal assistants."
    >
      <div>
        <Card
          title="My Assistants"
          extra={
            <Can permission={Permissions.AssistantsCreate}>
              <Tooltip title="Create assistant">
                <Button
                  type="text"
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
              <Empty description="No assistants yet — use the New Assistant button above to create one." />
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
                            <RobotOutlined />
                            <Text className="font-medium">{assistant.name}</Text>
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
        </Card>

        <AssistantFormDrawer />
      </div>
    </SettingsPageContainer>
  )
}
