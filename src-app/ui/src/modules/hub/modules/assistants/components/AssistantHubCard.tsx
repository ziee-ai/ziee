import { App, Card, Tag, Typography, Button, Flex } from 'antd'
import {
  InfoCircleOutlined,
  RobotOutlined,
  EyeOutlined,
} from '@ant-design/icons'
import { Permissions, type HubAssistant } from '@/api-client/types'
import { useState } from 'react'
import { AssistantDetailsDrawer } from '@/modules/hub/modules/assistants/components/AssistantDetailsDrawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { useNavigate } from 'react-router-dom'

const { Text } = Typography

interface AssistantHubCardProps {
  assistant: HubAssistant
}

export function AssistantHubCard({ assistant }: AssistantHubCardProps) {
  const { message } = App.useApp()
  const navigate = useNavigate()
  const [showDetails, setShowDetails] = useState(false)
  const [isCreating, setIsCreating] = useState(false)
  const [isCreatingTemplate, setIsCreatingTemplate] = useState(false)
  const canCreate = usePermission(Permissions.HubAssistantsCreate)
  const canCreateTemplate = usePermission(Permissions.AssistantsTemplateCreate)

  // Check if assistant was already created from this hub assistant
  const isAlreadyCreated =
    assistant.created_ids && assistant.created_ids.length > 0

  const handleUseAssistant = async () => {
    setIsCreating(true)
    try {
      // Create a user assistant from the hub assistant via store action
      await Stores.HubAssistants.createFromHub({
        hub_id: assistant.id,
        name: assistant.name,
        description: assistant.description,
        instructions: assistant.instructions,
        parameters: assistant.parameters,
        is_default: false,
        enabled: true,
      })

      message.success(
        `Assistant "${assistant.display_name}" created successfully!`,
      )

      // Navigate to the assistants settings page to see the created assistant
      navigate('/settings/assistants')
    } catch (error: any) {
      console.error('Failed to create assistant:', error)
      message.error(
        `Failed to create assistant: ${error.message || 'Unknown error'}`,
      )
    } finally {
      setIsCreating(false)
    }
  }

  const handleUseAsTemplate = async () => {
    setIsCreatingTemplate(true)
    try {
      // Install as a system-wide TEMPLATE (is_template=true, no
      // owner — enforced by the `template_must_have_no_owner` CHECK
      // constraint in migration 6). The clone-default-templates-
      // on-signup hook in the assistant module then propagates this
      // template to every new user's assistant list.
      await Stores.HubAssistants.createTemplateFromHub({
        hub_id: assistant.id,
        name: assistant.name,
        description: assistant.description,
        instructions: assistant.instructions,
        parameters: assistant.parameters,
        is_default: false,
        enabled: true,
      })

      message.success(
        `Template "${assistant.display_name}" created successfully!`,
      )

      // Navigate to the templates admin page so the admin can see it.
      navigate('/settings/assistant-templates')
    } catch (error: any) {
      console.error('Failed to create assistant template:', error)
      message.error(
        `Failed to create template: ${error.message || 'Unknown error'}`,
      )
    } finally {
      setIsCreatingTemplate(false)
    }
  }

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
        onClick={() => setShowDetails(true)}
        data-assistant-id={assistant.id}
        data-testid={`hub-assistant-card-${assistant.id}`}
      >
        <div className="flex items-start gap-3 flex-wrap">
          {/* Assistant Info */}
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <div className="flex-1 min-w-48">
                <Flex className="gap-2 items-center">
                  <RobotOutlined />
                  <Text className="font-medium cursor-pointer">
                    {assistant.display_name}
                  </Text>
                  {assistant.category && (
                    <Tag color="geekblue" className="text-xs">
                      {assistant.category}
                    </Tag>
                  )}
                  {isAlreadyCreated && <Tag color="green">Created</Tag>}
                  {(isCreating || isCreatingTemplate) && (
                    <Tag color="blue">
                      {isCreatingTemplate ? 'Creating template...' : 'Creating...'}
                    </Tag>
                  )}
                </Flex>
              </div>
              <div className="flex gap-1 items-center justify-end">
                <Button
                  icon={<InfoCircleOutlined />}
                  onClick={e => {
                    e.stopPropagation()
                    setShowDetails(true)
                  }}
                >
                  Details
                </Button>
                {isAlreadyCreated && (
                  <Button
                    icon={<EyeOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      navigate('/settings/assistants')
                    }}
                  >
                    View Assistant
                  </Button>
                )}
                {!isAlreadyCreated && canCreate && (
                  <Button
                    type="primary"
                    icon={<RobotOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      handleUseAssistant()
                    }}
                    loading={isCreating}
                    disabled={isCreating || isCreatingTemplate}
                  >
                    Use Assistant
                  </Button>
                )}
                {/* "Use as Template" — admin power-user action.
                    Shown WHENEVER the user has both permissions,
                    regardless of whether the per-user "Created"
                    badge is set (a personal install doesn't
                    preclude also installing as a template).
                    Default-styled so the primary "Use Assistant"
                    path stays visually dominant. The backend
                    requires both `hub::assistants::create` AND
                    `assistant_templates::create`. */}
                {canCreate && canCreateTemplate && (
                  <Button
                    icon={<RobotOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      handleUseAsTemplate()
                    }}
                    loading={isCreatingTemplate}
                    disabled={isCreating || isCreatingTemplate}
                    data-testid="hub-assistant-use-as-template-btn"
                  >
                    Use as Template
                  </Button>
                )}
              </div>
            </div>

            <div>
              {assistant.description && (
                <Text type="secondary" className="text-sm mb-2 block">
                  {assistant.description}
                </Text>
              )}

              {/* Tags */}
              {assistant.tags && assistant.tags.length > 0 && (
                <div className="mb-2">
                  <Text type="secondary" className="text-xs mr-2">
                    Tags:
                  </Text>
                  <Flex
                    wrap
                    className="gap-1"
                    style={{ display: 'inline-flex' }}
                  >
                    {assistant.tags.map(tag => (
                      <Tag key={tag} color="default" className="text-xs">
                        {tag}
                      </Tag>
                    ))}
                  </Flex>
                </div>
              )}

              {/* Metadata */}
              <div className="mb-2">
                <Flex wrap className="gap-4 text-xs">
                  {assistant.author && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Author:
                      </Text>{' '}
                      {assistant.author}
                    </span>
                  )}
                  {assistant.recommended_models &&
                    assistant.recommended_models.length > 0 && (
                      <span>
                        <Text type="secondary" className="text-xs">
                          Models:
                        </Text>{' '}
                        {assistant.recommended_models.slice(0, 2).join(', ')}
                        {assistant.recommended_models.length > 2 && '...'}
                      </span>
                    )}
                </Flex>
              </div>
            </div>
          </div>
        </div>
      </Card>

      <AssistantDetailsDrawer
        assistant={showDetails ? assistant : null}
        open={showDetails}
        onClose={() => setShowDetails(false)}
      />
    </>
  )
}
