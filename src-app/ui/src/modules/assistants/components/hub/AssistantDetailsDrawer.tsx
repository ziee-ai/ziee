import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Flex, Tag, Typography, Card } from 'antd'
import type { HubAssistant } from '@/api-client/types'

const { Title, Text } = Typography

interface AssistantDetailsDrawerProps {
  assistant: HubAssistant | null
  open: boolean
  onClose: () => void
}

export function AssistantDetailsDrawer({ assistant, open, onClose }: AssistantDetailsDrawerProps) {
  if (!assistant) return null

  return (
    <Drawer
      title={assistant.display_name}
      open={open}
      onClose={onClose}
    >
      <Flex vertical className="gap-4">
        {/* Basic Info */}
        <div>
          <Title level={3} className="!m-0 !mb-2">
            {assistant.display_name}
          </Title>
          <Text type="secondary" className="text-xs">
            {assistant.name}
          </Text>
          {assistant.description && (
            <div className="mt-2">
              <Text type="secondary">{assistant.description}</Text>
            </div>
          )}
        </div>

        {/* Instructions */}
        <div>
          <Title level={5}>Instructions</Title>
          <Card size="small" className="bg-gray-50">
            <Text className="text-sm whitespace-pre-wrap">
              {assistant.instructions}
            </Text>
          </Card>
        </div>

        {/* Use Cases */}
        {assistant.use_cases && assistant.use_cases.length > 0 && (
          <div>
            <Title level={5}>Use Cases</Title>
            <ul className="ml-4">
              {assistant.use_cases.map((useCase, idx) => (
                <li key={idx} className="text-sm">
                  {useCase}
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* Assistant Details */}
        <div>
          <Title level={5}>Details</Title>
          <Flex vertical className="gap-2">
            {assistant.author && (
              <Flex justify="space-between">
                <Text type="secondary">Author:</Text>
                <Text>{assistant.author}</Text>
              </Flex>
            )}
            {assistant.popularity_score && (
              <Flex justify="space-between">
                <Text type="secondary">Popularity Score:</Text>
                <Text>{assistant.popularity_score}</Text>
              </Flex>
            )}
          </Flex>
        </div>

        {/* Tags */}
        {assistant.tags && assistant.tags.length > 0 && (
          <div>
            <Title level={5}>Tags</Title>
            <Flex wrap className="gap-1">
              {assistant.tags.map(tag => (
                <Tag key={tag} color="default">
                  {tag}
                </Tag>
              ))}
            </Flex>
          </div>
        )}

        {/* Parameters */}
        {assistant.parameters && Object.keys(assistant.parameters).length > 0 && (
          <div>
            <Title level={5}>Parameters</Title>
            <Card size="small">
              <pre className="text-xs overflow-auto m-0">
                {JSON.stringify(assistant.parameters, null, 2)}
              </pre>
            </Card>
          </div>
        )}
      </Flex>
    </Drawer>
  )
}
