import { Trash2, Pencil, Plus, Bot } from 'lucide-react'
import {
  Button,
  Card,
  Descriptions,
  Separator,
  Empty,
  Flex,
  Confirm,
  Tag,
  Tooltip,
  Text,
} from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { useEffect } from 'react'
import { Stores } from '@/modules/assistant/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions, type Assistant } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { AssistantFormDrawer } from '@/modules/assistant/components/AssistantFormDrawer'

export function UserAssistantsSettings() {
  // Store state
  const { assistants: assistantsMap, loading, error } = Stores.UserAssistants

  const assistants = Array.from(assistantsMap.values())

  const canEdit = usePermission(Permissions.AssistantsEdit)
  const canDelete = usePermission(Permissions.AssistantsDelete)

  // Show errors
  useEffect(() => {
    if (error) {
      Stores.UserAssistants.clearUserAssistantsStoreError()
    }
  }, [error])

  const handleDelete = async (assistant: Assistant) => {
    try {
      await Stores.UserAssistants.deleteUserAssistant(assistant.id)
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
        <Card
          data-testid="user-assistants-card"
          title="My Assistants"
          extra={
            <Can permission={Permissions.AssistantsCreate}>
              <Tooltip content="Create assistant">
                <Button
                  data-testid="user-assistants-create-btn"
                  variant="ghost"
                  icon={<Plus aria-hidden="true" />}
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
              <Empty data-testid="user-assistants-empty" description="No assistants yet" />
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
                            <Bot />
                            <Text className="font-medium">{assistant.name}</Text>
                            {assistant.is_default && (
                              <Tag data-testid={`user-assistant-${assistant.id}-default-tag`} tone="success">Default</Tag>
                            )}
                            {!assistant.enabled && (
                              <Tag data-testid={`user-assistant-${assistant.id}-inactive-tag`} tone="error">Inactive</Tag>
                            )}
                          </Flex>
                        </div>
                        <div className="flex gap-1 items-center justify-end">
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
        </Card>

        <AssistantFormDrawer />
      </div>
    </SettingsPageContainer>
  )
}
