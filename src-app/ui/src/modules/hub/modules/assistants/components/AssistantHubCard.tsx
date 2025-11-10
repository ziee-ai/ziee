import { App, Card, Tag, Typography, Button, Flex } from 'antd'
import { InfoCircleOutlined, RobotOutlined, EyeOutlined } from '@ant-design/icons'
import type { HubAssistant } from '@/api-client/types'
import { useState } from 'react'
import { AssistantDetailsDrawer } from './AssistantDetailsDrawer'
import { Stores } from '@/core/stores'
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

  // Check if assistant was already created from this hub assistant
  const isAlreadyCreated = assistant.created_ids && assistant.created_ids.length > 0

  const handleUseAssistant = async () => {
    setIsCreating(true)
    try {
      // Create a user assistant based on the hub assistant
      await Stores.UserAssistants.createUserAssistant({
        name: assistant.name,
        description: assistant.description || '',
        instructions: assistant.instructions || '',
        parameters: assistant.parameters || {},
        is_default: false,
        enabled: true,
      })

      message.success(`Assistant "${assistant.display_name}" created successfully!`)

      // Navigate to /assistants after creation
      navigate('/assistants')
    } catch (error: any) {
      console.error('Failed to create assistant:', error)
      message.error(
        `Failed to create assistant: ${error.message || 'Unknown error'}`,
      )
    } finally {
      setIsCreating(false)
    }
  }

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
        onClick={() => setShowDetails(true)}
        data-assistant-id={assistant.id}
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
                  {isCreating && <Tag color="blue">Creating...</Tag>}
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
                <Button
                  type={isAlreadyCreated ? undefined : 'primary'}
                  icon={isAlreadyCreated ? <EyeOutlined /> : <RobotOutlined />}
                  onClick={e => {
                    e.stopPropagation()
                    if (isAlreadyCreated) {
                      navigate('/assistants')
                    } else {
                      handleUseAssistant()
                    }
                  }}
                  loading={isCreating}
                  disabled={isCreating}
                >
                  {isAlreadyCreated ? 'View Assistant' : 'Use Assistant'}
                </Button>
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
                  <Flex wrap className="gap-1" style={{ display: 'inline-flex' }}>
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
                  {assistant.recommended_models && assistant.recommended_models.length > 0 && (
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
